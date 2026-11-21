//! Phase 4A real timer engine.
//!
//! `imp` is the pure state machine: no Tauri or Win32 types, times are Unix
//! milliseconds from `SystemTime`, so a running timer keeps counting through
//! sleep and fires on wake. The layer below owns the shared engine, the 250 ms
//! tick thread, the completion alert/toast and settings persistence.

use serde_json::Value;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;

mod imp {
  use std::time::{SystemTime, UNIX_EPOCH};

  /// Shortest allowed timer (1 s).
  pub const MIN_DURATION_MS: u64 = 1_000;
  /// Longest allowed timer (23:59:59).
  pub const MAX_DURATION_MS: u64 = 86_399_000;
  /// Preset durations the UI offers; starting anything else is "custom".
  pub const PRESET_DURATIONS_MS: [u64; 3] = [5 * 60_000, 10 * 60_000, 25 * 60_000];
  /// Tick cadence of the background thread. The frontend counts down locally
  /// from `endsAtMs`; this only decides when the timer flips to done.
  pub const TICK_INTERVAL_MS: u64 = 250;

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum TimerState {
    Idle { duration_ms: u64 },
    Running { ends_at_ms: u64, total_ms: u64 },
    Paused { remaining_ms: u64, total_ms: u64 },
    Done { total_ms: u64 },
  }

  impl TimerState {
    /// Duration shown in idle / the total that was run.
    pub fn total_ms(&self) -> u64 {
      match *self {
        TimerState::Idle { duration_ms } => duration_ms,
        TimerState::Running { total_ms, .. }
        | TimerState::Paused { total_ms, .. }
        | TimerState::Done { total_ms } => total_ms,
      }
    }

    /// Wall-clock deadline, only while running.
    pub fn ends_at_ms(&self) -> Option<u64> {
      match *self {
        TimerState::Running { ends_at_ms, .. } => Some(ends_at_ms),
        _ => None,
      }
    }

    /// Remaining time. A running timer whose clock jumped backwards never
    /// reports more than its original total, so it cannot be extended.
    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
      match *self {
        TimerState::Idle { duration_ms } => duration_ms,
        TimerState::Running { ends_at_ms, total_ms } => {
          ends_at_ms.saturating_sub(now_ms).min(total_ms)
        }
        TimerState::Paused { remaining_ms, .. } => remaining_ms,
        TimerState::Done { .. } => 0,
      }
    }

    /// Idle only: replaces the configured duration.
    pub fn set_duration(&mut self, duration_ms: u64) -> bool {
      if matches!(self, TimerState::Idle { .. }) {
        *self = TimerState::Idle {
          duration_ms: clamp_duration(duration_ms),
        };
        true
      } else {
        false
      }
    }

    /// Idle only: starts from the configured duration.
    pub fn start(&mut self, now_ms: u64) -> bool {
      if let TimerState::Idle { duration_ms } = *self {
        *self = TimerState::Running {
          ends_at_ms: now_ms + duration_ms,
          total_ms: duration_ms,
        };
        true
      } else {
        false
      }
    }

    /// Freezes a running timer at the remaining time.
    pub fn pause(&mut self, now_ms: u64) -> bool {
      if let TimerState::Running {
        ends_at_ms,
        total_ms,
      } = *self
      {
        *self = TimerState::Paused {
          remaining_ms: ends_at_ms.saturating_sub(now_ms).min(total_ms),
          total_ms,
        };
        true
      } else {
        false
      }
    }

    /// Rebases the deadline at now + stored remaining.
    pub fn resume(&mut self, now_ms: u64) -> bool {
      if let TimerState::Paused {
        remaining_ms,
        total_ms,
      } = *self
      {
        *self = TimerState::Running {
          ends_at_ms: now_ms + remaining_ms,
          total_ms,
        };
        true
      } else {
        false
      }
    }

    /// Back to idle with whatever duration was last used.
    pub fn reset(&mut self) {
      *self = TimerState::Idle {
        duration_ms: clamp_duration(self.total_ms()),
      };
    }

    /// Done only: runs the same total again.
    pub fn restart(&mut self, now_ms: u64) -> bool {
      if let TimerState::Done { total_ms } = *self {
        *self = TimerState::Running {
          ends_at_ms: now_ms + total_ms,
          total_ms,
        };
        true
      } else {
        false
      }
    }

    /// Idle changes the duration, Running moves deadline and total, Paused
    /// changes remaining and total. Never below 1 s or above 23:59:59.
    pub fn adjust(&mut self, delta_ms: i64, now_ms: u64) -> bool {
      match *self {
        TimerState::Idle { .. } => {
          *self = TimerState::Idle {
            duration_ms: clamp_duration(offset(self.total_ms(), delta_ms)),
          };
          true
        }
        TimerState::Running { ends_at_ms, .. } => {
          let ends_at_ms = (ends_at_ms as i64)
            .saturating_add(delta_ms)
            .max(now_ms as i64) as u64;
          *self = TimerState::Running {
            ends_at_ms,
            total_ms: clamp_duration(offset(self.total_ms(), delta_ms)),
          };
          true
        }
        TimerState::Paused { .. } => {
          *self = TimerState::Paused {
            remaining_ms: clamp_duration(offset(self.remaining_ms(now_ms), delta_ms)),
            total_ms: clamp_duration(offset(self.total_ms(), delta_ms)),
          };
          true
        }
        TimerState::Done { .. } => false,
      }
    }

    /// Returns `Some(Done)` exactly once, on the first tick at/after zero.
    pub fn tick(&mut self, now_ms: u64) -> Option<TimerState> {
      match *self {
        TimerState::Running { ends_at_ms, total_ms } if now_ms >= ends_at_ms => {
          *self = TimerState::Done { total_ms };
          Some(TimerState::Done { total_ms })
        }
        _ => None,
      }
    }
  }

  /// Unix milliseconds since the epoch; 0 if the wall clock is before it.
  pub fn wall_clock_ms() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|elapsed| elapsed.as_millis() as u64)
      .unwrap_or(0)
  }

  pub fn clamp_duration(duration_ms: u64) -> u64 {
    duration_ms.clamp(MIN_DURATION_MS, MAX_DURATION_MS)
  }

  pub fn is_preset_duration(duration_ms: u64) -> bool {
    PRESET_DURATIONS_MS.contains(&duration_ms)
  }

  /// Only non-preset starts are remembered as "last custom".
  pub fn records_last_custom(duration_ms: u64) -> bool {
    !is_preset_duration(duration_ms)
  }

  /// Signed addition that cannot underflow or overflow.
  pub fn offset(value: u64, delta_ms: i64) -> u64 {
    if delta_ms >= 0 {
      value.saturating_add(delta_ms as u64)
    } else {
      value.saturating_sub(delta_ms.unsigned_abs())
    }
  }

  /// `M:SS`, or `H:MM:SS` past an hour (toast text).
  pub fn format_duration(duration_ms: u64) -> String {
    let total_seconds = duration_ms / 1_000;
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
      format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
      format!("{minutes}:{seconds:02}")
    }
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    const NOW: u64 = 10_000;

    #[test]
    fn start_then_tick_to_zero_fires_done_exactly_once() {
      let mut state = TimerState::Idle { duration_ms: 5_000 };
      assert!(state.start(NOW));
      assert_eq!(
        state,
        TimerState::Running {
          ends_at_ms: NOW + 5_000,
          total_ms: 5_000
        }
      );
      // One millisecond early is still running.
      assert_eq!(state.tick(NOW + 4_999), None);
      assert_eq!(
        state.tick(NOW + 5_000),
        Some(TimerState::Done { total_ms: 5_000 })
      );
      // Further ticks do not fire again.
      assert_eq!(state.tick(NOW + 6_000), None);
      assert_eq!(state.tick(u64::MAX), None);
    }

    #[test]
    fn pause_stores_remaining_and_resume_rebases_ends_at() {
      let mut state = TimerState::Idle { duration_ms: 10_000 };
      state.start(NOW);
      assert!(state.pause(NOW + 3_000));
      assert_eq!(
        state,
        TimerState::Paused {
          remaining_ms: 7_000,
          total_ms: 10_000
        }
      );
      assert_eq!(state.remaining_ms(NOW + 99_000), 7_000);
      assert!(state.resume(NOW + 8_000));
      assert_eq!(
        state,
        TimerState::Running {
          ends_at_ms: NOW + 15_000,
          total_ms: 10_000
        }
      );
    }

    #[test]
    fn adjust_idle_running_and_paused() {
      let mut state = TimerState::Idle { duration_ms: 10_000 };
      assert!(state.adjust(5_000, NOW));
      assert_eq!(state.total_ms(), 15_000);

      assert!(state.start(NOW));
      assert!(state.adjust(10_000, NOW));
      assert_eq!(
        state,
        TimerState::Running {
          ends_at_ms: NOW + 25_000,
          total_ms: 25_000
        }
      );

      assert!(state.pause(NOW));
      assert!(state.adjust(-5_000, NOW));
      assert_eq!(
        state,
        TimerState::Paused {
          remaining_ms: 20_000,
          total_ms: 20_000
        }
      );
    }

    #[test]
    fn adjust_below_one_second_clamps_to_the_minimum() {
      let mut state = TimerState::Idle { duration_ms: 3_000 };
      assert!(state.adjust(-60_000, NOW));
      assert_eq!(state, TimerState::Idle { duration_ms: MIN_DURATION_MS });

      state.start(NOW);
      assert!(state.adjust(-60_000, NOW));
      assert_eq!(state.total_ms(), MIN_DURATION_MS);
      // The moved deadline can never be in the past.
      assert_eq!(state.ends_at_ms(), Some(NOW));

      assert!(state.pause(NOW));
      assert!(state.adjust(-60_000, NOW));
      assert_eq!(state.remaining_ms(NOW), MIN_DURATION_MS);
      assert_eq!(state.total_ms(), MIN_DURATION_MS);
    }

    #[test]
    fn duration_is_capped_at_23_59_59() {
      let mut state = TimerState::Idle { duration_ms: 10_000 };
      assert!(state.set_duration(24 * 60 * 60 * 1000));
      assert_eq!(state.total_ms(), MAX_DURATION_MS);
      assert!(state.adjust(i64::MAX, NOW));
      assert_eq!(state.total_ms(), MAX_DURATION_MS);
    }

    #[test]
    fn backwards_clock_jump_does_not_extend_the_timer() {
      let mut state = TimerState::Idle { duration_ms: 5_000 };
      state.start(NOW); // ends at NOW + 5_000
      // The machine woke up with the wall clock an hour behind.
      assert_eq!(state.tick(NOW.saturating_sub(3_600_000)), None);
      assert_eq!(state.remaining_ms(NOW.saturating_sub(3_600_000)), 5_000);
      // It still completes on the original deadline, not a shifted one.
      assert_eq!(state.tick(NOW + 4_999), None);
      assert_eq!(
        state.tick(NOW + 5_000),
        Some(TimerState::Done { total_ms: 5_000 })
      );
    }

    #[test]
    fn reset_returns_to_idle_with_the_last_duration() {
      let mut state = TimerState::Idle { duration_ms: 7_000 };
      state.start(NOW);
      state.pause(NOW + 2_000);
      state.reset();
      assert_eq!(state, TimerState::Idle { duration_ms: 7_000 });
      state.reset();
      assert_eq!(state, TimerState::Idle { duration_ms: 7_000 });
    }

    #[test]
    fn restart_from_done_runs_the_same_duration() {
      let mut state = TimerState::Idle { duration_ms: 9_000 };
      assert!(!state.restart(NOW)); // only valid from Done
      state.start(NOW);
      state.tick(NOW + 9_000);
      assert_eq!(state, TimerState::Done { total_ms: 9_000 });
      assert!(state.restart(NOW + 20_000));
      assert_eq!(
        state,
        TimerState::Running {
          ends_at_ms: NOW + 29_000,
          total_ms: 9_000
        }
      );
    }

    #[test]
    fn only_non_preset_starts_are_recorded_as_last_custom() {
      for preset in PRESET_DURATIONS_MS {
        assert!(!records_last_custom(preset));
      }
      assert!(records_last_custom(1));
      assert!(records_last_custom(7 * 60_000));
      assert!(records_last_custom(60 * 60 * 1000));
    }

    #[test]
    fn format_duration_matches_the_toast_text() {
      assert_eq!(format_duration(65_000), "1:05");
      assert_eq!(format_duration(3_600_000), "1:00:00");
      assert_eq!(format_duration(23 * 60 * 60 * 1000 + 59 * 60 * 1000 + 59_000), "23:59:59");
    }
  }
}

fn lock_engine(engine: &Mutex<TimerEngine>) -> MutexGuard<'_, TimerEngine> {
  match engine.lock() {
    Ok(guard) => guard,
    Err(poisoned) => poisoned.into_inner(),
  }
}

/// Shared engine behind every `timer_*` command and the tick thread.
pub struct TimerEngine {
  state: imp::TimerState,
  sound_on: bool,
  /// Last non-preset duration that was started (mirrors settings).
  last_custom_ms: Option<u64>,
}

impl TimerEngine {
  pub fn new(duration_ms: u64, sound_on: bool, last_custom_ms: Option<u64>) -> Self {
    Self {
      state: imp::TimerState::Idle {
        duration_ms: imp::clamp_duration(duration_ms),
      },
      sound_on,
      last_custom_ms: last_custom_ms.map(imp::clamp_duration),
    }
  }

  pub fn last_custom_ms(&self) -> Option<u64> {
    self.last_custom_ms
  }

  pub fn set_last_custom_ms(&mut self, duration_ms: Option<u64>) {
    self.last_custom_ms = duration_ms.map(imp::clamp_duration);
  }

  pub fn state(&self) -> imp::TimerState {
    self.state
  }

  pub fn sound_enabled(&self) -> bool {
    self.sound_on
  }

  pub fn set_sound_enabled(&mut self, enabled: bool) {
    self.sound_on = enabled;
  }

  pub fn tick(&mut self, now_ms: u64) -> Option<imp::TimerState> {
    self.state.tick(now_ms)
  }
}

fn snapshot_payload(state: &imp::TimerState, now_ms: u64, last_custom_ms: Option<u64>) -> Value {
  let (name, duration_ms, remaining_ms) = match *state {
    imp::TimerState::Idle { duration_ms } => ("idle", duration_ms, duration_ms),
    imp::TimerState::Running {
      ends_at_ms,
      total_ms,
    } => (
      "running",
      total_ms,
      ends_at_ms.saturating_sub(now_ms).min(total_ms),
    ),
    imp::TimerState::Paused {
      remaining_ms,
      total_ms,
    } => ("paused", total_ms, remaining_ms),
    imp::TimerState::Done { total_ms } => ("done", total_ms, 0),
  };
  let total_ms = state.total_ms();

  let mut payload = serde_json::json!({
    "state": name,
    "durationMs": duration_ms,
    "totalMs": total_ms,
    "remainingMs": remaining_ms,
    "capturedAtMs": now_ms,
    "lastCustomMs": last_custom_ms,
  });
  if let Some(ends_at_ms) = state.ends_at_ms() {
    payload["endsAtMs"] = Value::from(ends_at_ms);
  }
  payload
}

fn emit_state(app: &AppHandle, engine: &TimerEngine) {
  let payload = snapshot_payload(
    &engine.state(),
    imp::wall_clock_ms(),
    engine.last_custom_ms(),
  );
  if let Err(err) = app.emit("timer-state", payload) {
    log::warn!("timer: failed to emit timer-state: {err}");
  }
}

/// Small settings mutation; failures are logged, the timer keeps working.
fn persist<F>(app: &AppHandle, update: F)
where
  F: FnOnce(&mut crate::settings::AppSettings),
{
  let result = (|| -> Result<(), String> {
    let mut settings = crate::settings::load_or_default(app);
    update(&mut settings);
    crate::settings::save_settings(app, &settings)
  })();
  if let Err(err) = result {
    log::warn!("timer: failed to save timer settings: {err}");
  }
}

/// Native toast; a failure must never break the timer.
fn show_done_toast(app: &AppHandle, total_ms: u64) {
  let body = imp::format_duration(total_ms);
  if let Err(err) = app
    .notification()
    .builder()
    .title("Timer done")
    .body(body)
    .show()
  {
    log::warn!("timer: failed to show done toast: {err}");
  }
}

/// Three system beeps, 600 ms apart.
#[cfg(target_os = "windows")]
fn play_alert() {
  std::thread::spawn(|| {
    use windows::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows::Win32::UI::WindowsAndMessaging::MB_ICONASTERISK;
    for index in 0..3 {
      if unsafe { MessageBeep(MB_ICONASTERISK) }.is_err() {
        log::warn!("timer: MessageBeep failed");
      }
      if index < 2 {
        std::thread::sleep(Duration::from_millis(600));
      }
    }
  });
}

#[cfg(not(target_os = "windows"))]
fn play_alert() {}

/// Shared state; returns the fresh snapshot for the command response.
fn with_snapshot<F>(
  app: &AppHandle,
  engine: &Arc<Mutex<TimerEngine>>,
  mutate: F,
) -> Value
where
  F: FnOnce(&mut TimerEngine, u64),
{
  let now_ms = imp::wall_clock_ms();
  let payload = {
    let mut guard = lock_engine(engine);
    mutate(&mut guard, now_ms);
    snapshot_payload(&guard.state(), now_ms, guard.last_custom_ms())
  };
  emit_state(app, &lock_engine(engine));
  payload
}

#[tauri::command]
pub fn timer_get(engine: State<'_, Arc<Mutex<TimerEngine>>>) -> Result<Value, String> {
  let guard = lock_engine(engine.inner());
  Ok(snapshot_payload(
    &guard.state(),
    imp::wall_clock_ms(),
    guard.last_custom_ms(),
  ))
}

#[tauri::command]
pub fn timer_set(
  app: AppHandle,
  engine: State<'_, Arc<Mutex<TimerEngine>>>,
  duration_ms: u64,
) -> Result<Value, String> {
  let clamped = imp::clamp_duration(duration_ms);
  let payload = with_snapshot(&app, engine.inner(), |timer, _now_ms| {
    timer.state.set_duration(clamped);
  });
  persist(&app, |settings| settings.timer_duration_ms = clamped);
  Ok(payload)
}

#[tauri::command]
pub fn timer_start(
  app: AppHandle,
  engine: State<'_, Arc<Mutex<TimerEngine>>>,
) -> Result<Value, String> {
  let now_ms = imp::wall_clock_ms();
  let (payload, started_custom) = {
    let mut guard = lock_engine(engine.inner());
    let mut started_custom = None;
    if guard.state.start(now_ms) {
      let total_ms = guard.state.total_ms();
      if imp::records_last_custom(total_ms) {
        guard.set_last_custom_ms(Some(total_ms));
        started_custom = Some(total_ms);
      }
    }
    (
      snapshot_payload(&guard.state(), now_ms, guard.last_custom_ms()),
      started_custom,
    )
  };
  if let Some(last_custom) = started_custom {
    persist(&app, |settings| settings.timer_last_custom_ms = Some(last_custom));
  }
  emit_state(&app, &lock_engine(engine.inner()));
  Ok(payload)
}

#[tauri::command]
pub fn timer_pause(
  app: AppHandle,
  engine: State<'_, Arc<Mutex<TimerEngine>>>,
) -> Result<Value, String> {
  Ok(with_snapshot(&app, engine.inner(), |timer, now_ms| {
    timer.state.pause(now_ms);
  }))
}

#[tauri::command]
pub fn timer_resume(
  app: AppHandle,
  engine: State<'_, Arc<Mutex<TimerEngine>>>,
) -> Result<Value, String> {
  Ok(with_snapshot(&app, engine.inner(), |timer, now_ms| {
    timer.state.resume(now_ms);
  }))
}

#[tauri::command]
pub fn timer_reset(
  app: AppHandle,
  engine: State<'_, Arc<Mutex<TimerEngine>>>,
) -> Result<Value, String> {
  Ok(with_snapshot(&app, engine.inner(), |timer, _now_ms| {
    timer.state.reset();
  }))
}

#[tauri::command]
pub fn timer_adjust(
  app: AppHandle,
  engine: State<'_, Arc<Mutex<TimerEngine>>>,
  delta_ms: i64,
) -> Result<Value, String> {
  let now_ms = imp::wall_clock_ms();
  let (payload, idle_duration) = {
    let mut guard = lock_engine(engine.inner());
    let was_idle = matches!(guard.state, imp::TimerState::Idle { .. });
    guard.state.adjust(delta_ms, now_ms);
    let idle_duration = if was_idle {
      Some(guard.state.total_ms())
    } else {
      None
    };
    (
      snapshot_payload(&guard.state(), now_ms, guard.last_custom_ms()),
      idle_duration,
    )
  };
  if let Some(duration) = idle_duration {
    persist(&app, |settings| settings.timer_duration_ms = duration);
  }
  emit_state(&app, &lock_engine(engine.inner()));
  Ok(payload)
}

#[tauri::command]
pub fn timer_restart(
  app: AppHandle,
  engine: State<'_, Arc<Mutex<TimerEngine>>>,
) -> Result<Value, String> {
  Ok(with_snapshot(&app, engine.inner(), |timer, now_ms| {
    timer.state.restart(now_ms);
  }))
}

/// Called by `settings_set` so a saved toggle applies without a restart.
pub fn apply_sound_setting(app: &AppHandle, enabled: bool) {
  if let Some(engine) = app.try_state::<Arc<Mutex<TimerEngine>>>() {
    lock_engine(engine.inner()).set_sound_enabled(enabled);
  }
}

/// Spawns the single tick thread. Non-Windows builds share the same loop; only
/// the alert/toast side effects are cfg-gated.
pub fn start(
  app: AppHandle,
  duration_ms: u64,
  sound_on: bool,
  last_custom_ms: Option<u64>,
) {
  let shared = Arc::new(Mutex::new(TimerEngine::new(
    duration_ms,
    sound_on,
    last_custom_ms,
  )));
  app.manage(shared.clone());

  let spawned = std::thread::Builder::new()
    .name("float-timer".into())
    .spawn(move || loop {
      std::thread::sleep(Duration::from_millis(imp::TICK_INTERVAL_MS));
      let now_ms = imp::wall_clock_ms();
      let completed = {
        let mut guard = lock_engine(&shared);
        guard.tick(now_ms)
      };
      let Some(done) = completed else {
        continue;
      };
      let total_ms = done.total_ms();
      if let Err(err) = app.emit("timer-done", serde_json::json!({ "totalMs": total_ms })) {
        log::warn!("timer: failed to emit timer-done: {err}");
      }
      let guard = lock_engine(&shared);
      emit_state(&app, &guard);
      let sound_on = guard.sound_enabled();
      drop(guard);
      if sound_on {
        play_alert();
      }
      show_done_toast(&app, total_ms);
    });
  if let Err(err) = spawned {
    log::warn!("timer: failed to start tick thread: {err}");
  }
}
