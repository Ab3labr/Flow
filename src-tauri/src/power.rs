//! AC/battery transitions for the compact charging activity (Phase 3D-4).
//!
//! The Win32 read lives behind `#[cfg(windows)]`; the decision logic below is
//! pure and unit-tested on every platform. The poll thread keeps a baseline
//! (the startup snapshot, which is never emitted) and emits a real AC change on
//! the poll that sees it. Further AC changes are held for a short flicker guard
//! so a loose cable does not flash the bar twice; the final reading wins.

use serde::{Deserialize, Serialize};

/// `GetSystemPowerStatus` poll cadence. Fast enough that a plug is acted on in
/// the same poll instead of waiting out a 2 s window.
pub const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
/// After an emitted AC change, further AC changes are held this long so a
/// brief flicker collapses into one corrected final event.
pub const FLICKER_GUARD_MS: u64 = 800;

const AC_OFFLINE: u8 = 0;
const AC_ONLINE: u8 = 1;
const BATTERY_FLAG_CHARGING: u8 = 1 << 3;
const BATTERY_FLAG_NO_BATTERY: u8 = 1 << 7;
const PERCENT_UNKNOWN: u8 = 255;

/// Raw fields from `SYSTEM_POWER_STATUS`, independent of Win32 types so tests
/// can drive the decision logic directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawPowerStatus {
  pub ac_line_status: u8,
  pub battery_flag: u8,
  pub battery_life_percent: u8,
}

/// The only parts of a reading that may trigger an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerSignal {
  pub plugged_in: bool,
  pub percent: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PowerReading {
  signal: PowerSignal,
  charging: bool,
}

impl PowerReading {
  /// `None` means "never emit": no system battery or unknown AC status.
  fn from_raw(raw: RawPowerStatus) -> Option<Self> {
    if raw.battery_flag & BATTERY_FLAG_NO_BATTERY != 0 {
      return None;
    }
    match raw.ac_line_status {
      AC_OFFLINE | AC_ONLINE => {}
      // 255 is `UNKNOWN`; any other value is not a usable AC state either.
      _ => return None,
    }
    let percent = (raw.battery_life_percent != PERCENT_UNKNOWN &&
      raw.battery_life_percent <= 100)
      .then_some(raw.battery_life_percent);
    Some(Self {
      signal: PowerSignal {
        plugged_in: raw.ac_line_status == AC_ONLINE,
        percent,
      },
      charging: raw.battery_flag & BATTERY_FLAG_CHARGING != 0,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PowerTransition {
  Plugged,
  Unplugged,
  None,
}

/// Rust → frontend `power-state` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerPayload {
  pub plugged_in: bool,
  pub charging: bool,
  pub percent: Option<u8>,
  pub transition: PowerTransition,
}

/// All state needed to decide the next event. Copyable so `decide` can stay
/// pure; the thread just stores the returned state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PowerDeciderState {
  /// Last emitted signal (or the silent startup baseline).
  pub baseline: Option<PowerSignal>,
  /// While set, further *AC* changes are held until this timestamp. Percent
  /// changes are never held: they refresh the bar without a transition.
  pub guard_until_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
  pub state: PowerDeciderState,
  pub emit: Option<PowerPayload>,
}

/// Pure decision step.
///
/// The first usable reading becomes the baseline and is never emitted. After
/// that a changed signal is emitted on the same poll. If the change flips the
/// AC line, an 800 ms guard starts; while it is active another AC flip is held.
/// When the guard expires the latest reading wins: if it still differs from the
/// emitted baseline, one corrected event is emitted. A charging-bit-only change
/// never moves the signal, so it never emits.
pub fn decide(state: PowerDeciderState, raw: RawPowerStatus, now_ms: u64) -> Decision {
  let Some(reading) = PowerReading::from_raw(raw) else {
    // No battery (or unknown AC): reset so a later first reading is also
    // treated as an un-emitted baseline.
    return Decision {
      state: PowerDeciderState::default(),
      emit: None,
    };
  };
  let signal = reading.signal;

  let Some(baseline) = state.baseline else {
    return Decision {
      state: PowerDeciderState {
        baseline: Some(signal),
        guard_until_ms: None,
      },
      emit: None,
    };
  };

  if signal == baseline {
    // Nothing usable changed, or only the charging bit moved. Keep an active
    // guard so a stable poll in the middle of a flicker cannot shorten it.
    return Decision {
      state: PowerDeciderState {
        baseline: Some(baseline),
        guard_until_ms: state.guard_until_ms.filter(|until| now_ms < *until),
      },
      emit: None,
    };
  }

  let ac_changed = signal.plugged_in != baseline.plugged_in;
  let guarded = state.guard_until_ms.is_some_and(|until| now_ms < until);

  // A second AC flip inside the guard is held; the final reading wins.
  if ac_changed && guarded {
    return Decision { state, emit: None };
  }

  let transition = if ac_changed {
    if signal.plugged_in {
      PowerTransition::Plugged
    } else {
      PowerTransition::Unplugged
    }
  } else {
    PowerTransition::None
  };
  let guard_until_ms = if ac_changed {
    Some(now_ms + FLICKER_GUARD_MS)
  } else {
    // Percent-only updates leave any still-active guard alone.
    state.guard_until_ms.filter(|until| now_ms < *until)
  };

  Decision {
    state: PowerDeciderState {
      baseline: Some(signal),
      guard_until_ms,
    },
    emit: Some(PowerPayload {
      plugged_in: signal.plugged_in,
      charging: reading.charging,
      percent: signal.percent,
      transition,
    }),
  }
}

/// Tiny wrapper for the poll thread; `observe` is deterministic and emits no
/// side effects of its own.
#[derive(Debug, Default)]
pub struct PowerDecider {
  state: PowerDeciderState,
}

impl PowerDecider {
  pub fn observe(&mut self, raw: RawPowerStatus, now_ms: u64) -> Option<PowerPayload> {
    let decision = decide(self.state, raw, now_ms);
    self.state = decision.state;
    decision.emit
  }

  /// Last emitted/baseline signal, used by the debug stage log.
  pub fn baseline(&self) -> Option<PowerSignal> {
    self.state.baseline
  }
}

/// Read `GetSystemPowerStatus` every 500 ms on its own thread. Non-Windows
/// builds get a no-op so the rest of the app compiles and tests unchanged.
#[cfg(windows)]
pub fn start(app: tauri::AppHandle) {
  use tauri::{Emitter, Manager};
  use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

  let debug = crate::music::debug_enabled();
  std::thread::Builder::new()
    .name("float-power".into())
    .spawn(move || {
      // Let the webview gate its own stage logs with the same FLOAT_DEBUG=1.
      // Power events are rare, so the page is always ready by the first one.
      if debug {
        if let Some(window) = app.get_webview_window("main") {
          let _ = window.eval("window.FLOAT_DEBUG = '1';");
        }
      }

      let mut decider = PowerDecider::default();
      let started = std::time::Instant::now();
      let mut last_stage_ms = 0u64;

      loop {
        let mut status = SYSTEM_POWER_STATUS::default();
        if unsafe { GetSystemPowerStatus(&mut status) }.is_ok() {
          let raw = RawPowerStatus {
            ac_line_status: status.ACLineStatus,
            battery_flag: status.BatteryFlag,
            battery_life_percent: status.BatteryLifePercent,
          };
          let now_ms = started.elapsed().as_millis() as u64;

          if debug {
            if let Some(baseline) = decider.baseline() {
              let valid = raw.battery_flag & BATTERY_FLAG_NO_BATTERY == 0
                && (raw.ac_line_status == AC_OFFLINE || raw.ac_line_status == AC_ONLINE);
              let ac = raw.ac_line_status == AC_ONLINE;
              if valid && ac != baseline.plugged_in {
                log::info!(
                  "[power] stage 1 AC change detected t={now_ms}ms delta={}ms ac={ac}",
                  now_ms.saturating_sub(last_stage_ms)
                );
                last_stage_ms = now_ms;
              }
            }
          }

          if let Some(payload) = decider.observe(raw, now_ms) {
            if debug {
              log::info!(
                "[power] stage 2 emitted t={now_ms}ms delta={}ms transition={:?} percent={:?}",
                now_ms.saturating_sub(last_stage_ms),
                payload.transition,
                payload.percent
              );
              last_stage_ms = now_ms;
            }
            if let Err(err) = app.emit("power-state", payload) {
              log::warn!("failed to emit power-state: {err}");
            }
          }
        } else {
          log::warn!("GetSystemPowerStatus failed");
        }
        std::thread::sleep(POLL_INTERVAL);
      }
    })
    .expect("failed to spawn power thread");
}

#[cfg(not(windows))]
pub fn start(_app: tauri::AppHandle) {}

#[cfg(test)]
mod tests {
  use super::*;

  fn raw(ac: u8, flag: u8, percent: u8) -> RawPowerStatus {
    RawPowerStatus {
      ac_line_status: ac,
      battery_flag: flag,
      battery_life_percent: percent,
    }
  }

  fn initial(decider: &mut PowerDecider, status: RawPowerStatus) {
    assert_eq!(decider.observe(status, 0), None);
  }

  #[test]
  fn initial_state_is_never_emitted() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));
    assert_eq!(decider.observe(raw(AC_OFFLINE, 0, 50), 500), None);
    assert_eq!(
      decider.baseline(),
      Some(PowerSignal {
        plugged_in: false,
        percent: Some(50),
      })
    );
  }

  #[test]
  fn plugging_in_emits_on_the_first_poll() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    assert_eq!(
      decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 500),
      Some(PowerPayload {
        plugged_in: true,
        charging: true,
        percent: Some(50),
        transition: PowerTransition::Plugged,
      })
    );
  }

  #[test]
  fn unplugging_emits_on_the_first_poll() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 80));

    assert_eq!(
      decider.observe(raw(AC_OFFLINE, 0, 80), 500),
      Some(PowerPayload {
        plugged_in: false,
        charging: false,
        percent: Some(80),
        transition: PowerTransition::Unplugged,
      })
    );
  }

  #[test]
  fn flicker_within_the_guard_ends_in_the_final_state() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    // First plug is immediate and opens an 800 ms guard.
    assert!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 500).is_some());
    // Stable poll keeps the guard (it must not be shortened).
    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 700), None);
    // A flip inside the guard is held.
    assert_eq!(decider.observe(raw(AC_OFFLINE, 0, 50), 1000), None);
    // Guard expired: the latest reading wins with one corrected event.
    assert_eq!(
      decider.observe(raw(AC_OFFLINE, 0, 50), 1300),
      Some(PowerPayload {
        plugged_in: false,
        charging: false,
        percent: Some(50),
        transition: PowerTransition::Unplugged,
      })
    );
  }

  #[test]
  fn flicker_that_reverts_before_the_guard_emits_nothing_extra() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    assert!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 500).is_some());
    assert_eq!(decider.observe(raw(AC_OFFLINE, 0, 50), 900), None);
    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 1200), None);
    // Guard expiry with the original (plugged) state stays silent.
    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 1400), None);
  }

  #[test]
  fn percent_change_emits_none_immediately() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    assert_eq!(
      decider.observe(raw(AC_OFFLINE, 0, 49), 500),
      Some(PowerPayload {
        plugged_in: false,
        charging: false,
        percent: Some(49),
        transition: PowerTransition::None,
      })
    );
  }

  #[test]
  fn charging_flag_alone_does_not_emit() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_ONLINE, 0, 50));

    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 500), None);
    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 9_000), None);
  }

  #[test]
  fn unknown_percent_is_null() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    assert_eq!(
      decider.observe(raw(AC_OFFLINE, 0, PERCENT_UNKNOWN), 500),
      Some(PowerPayload {
        plugged_in: false,
        charging: false,
        percent: None,
        transition: PowerTransition::None,
      })
    );
  }

  #[test]
  fn unknown_ac_status_is_ignored_and_resets_the_baseline() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    assert_eq!(decider.observe(raw(255, 0, 49), 500), None);
    assert_eq!(decider.observe(raw(255, 0, 49), 9_000), None);
    // The next valid reading is a fresh baseline, so it is also silent.
    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 49), 9_500), None);
    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 49), 10_000), None);
  }

  #[test]
  fn no_battery_never_emits() {
    let no_battery = BATTERY_FLAG_NO_BATTERY;
    let mut decider = PowerDecider::default();
    assert_eq!(decider.observe(raw(AC_ONLINE, no_battery, 100), 0), None);

    // Even after a valid baseline, losing the battery resets and stays silent.
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));
    assert_eq!(decider.observe(raw(AC_ONLINE, no_battery, 50), 500), None);
    assert_eq!(decider.observe(raw(AC_ONLINE, no_battery, 49), 1_000), None);
  }

  #[test]
  fn both_ac_and_percent_change_prefer_the_transition() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    assert_eq!(
      decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 51), 500),
      Some(PowerPayload {
        plugged_in: true,
        charging: true,
        percent: Some(51),
        transition: PowerTransition::Plugged,
      })
    );
  }

  #[test]
  fn decision_is_pure_for_the_same_input() {
    let state = PowerDeciderState {
      baseline: Some(PowerSignal {
        plugged_in: false,
        percent: Some(50),
      }),
      guard_until_ms: None,
    };
    let status = raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50);
    assert_eq!(decide(state, status, 1_000), decide(state, status, 1_000));
  }
}
