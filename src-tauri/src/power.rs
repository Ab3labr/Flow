//! AC/battery transitions for the compact charging activity (Phase 3D-4).
//!
//! The Win32 read lives behind `#[cfg(windows)]`; the decision logic below is
//! pure and unit-tested on every platform. The poll thread keeps a baseline
//! (the startup snapshot, which is never emitted) and only emits after a
//! changed AC state or percentage has held for a full second.

use serde::{Deserialize, Serialize};

/// `GetSystemPowerStatus` poll cadence.
pub const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
/// A change must hold this long before it is emitted.
pub const CHANGE_HOLD_MS: u64 = 1_000;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingChange {
  pub signal: PowerSignal,
  /// Latest charging bit seen while this change was holding.
  pub charging: bool,
  pub since_ms: u64,
}

/// All state needed to decide the next event. Copyable so `decide` can stay
/// pure; the thread just stores the returned state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PowerDeciderState {
  pub baseline: Option<PowerSignal>,
  pub pending: Option<PendingChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
  pub state: PowerDeciderState,
  pub emit: Option<PowerPayload>,
}

/**
 * Pure decision step. The first usable reading becomes the baseline and is
 * never emitted. Only AC and percent changes start a pending change; a
 * charging-bit-only change is ignored until one of those actually moves.
 */
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
        pending: None,
      },
      emit: None,
    };
  };

  if signal == baseline {
    return Decision {
      state: PowerDeciderState {
        baseline: Some(baseline),
        pending: None,
      },
      emit: None,
    };
  }

  let pending = match state.pending {
    Some(pending) if pending.signal == signal => PendingChange {
      charging: reading.charging,
      ..pending
    },
    _ => PendingChange {
      signal,
      charging: reading.charging,
      since_ms: now_ms,
    },
  };

  if now_ms.saturating_sub(pending.since_ms) < CHANGE_HOLD_MS {
    return Decision {
      state: PowerDeciderState {
        baseline: Some(baseline),
        pending: Some(pending),
      },
      emit: None,
    };
  }

  let transition = if baseline.plugged_in != signal.plugged_in {
    if signal.plugged_in {
      PowerTransition::Plugged
    } else {
      PowerTransition::Unplugged
    }
  } else {
    PowerTransition::None
  };

  Decision {
    state: PowerDeciderState {
      baseline: Some(signal),
      pending: None,
    },
    emit: Some(PowerPayload {
      plugged_in: signal.plugged_in,
      charging: pending.charging,
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
}

/// Read `GetSystemPowerStatus` every two seconds on its own thread. Non-Windows
/// builds get a no-op so the rest of the app compiles and tests unchanged.
#[cfg(windows)]
pub fn start(app: tauri::AppHandle) {
  use tauri::Emitter;
  use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

  std::thread::Builder::new()
    .name("float-power".into())
    .spawn(move || {
      let mut decider = PowerDecider::default();
      let started = std::time::Instant::now();

      loop {
        let mut status = SYSTEM_POWER_STATUS::default();
        if unsafe { GetSystemPowerStatus(&mut status) }.is_ok() {
          let raw = RawPowerStatus {
            ac_line_status: status.ACLineStatus,
            battery_flag: status.BatteryFlag,
            battery_life_percent: status.BatteryLifePercent,
          };
          let now_ms = started.elapsed().as_millis() as u64;
          if let Some(payload) = decider.observe(raw, now_ms) {
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
    assert_eq!(decider.observe(raw(AC_OFFLINE, 0, 50), 2_000), None);
  }

  #[test]
  fn plugging_in_emits_after_the_hold() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 2_000), None);
    assert_eq!(
      decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 3_000),
      Some(PowerPayload {
        plugged_in: true,
        charging: true,
        percent: Some(50),
        transition: PowerTransition::Plugged,
      })
    );
  }

  #[test]
  fn unplugging_emits_after_the_hold() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 80));

    assert_eq!(decider.observe(raw(AC_OFFLINE, 0, 80), 2_000), None);
    assert_eq!(
      decider.observe(raw(AC_OFFLINE, 0, 80), 3_000),
      Some(PowerPayload {
        plugged_in: false,
        charging: false,
        percent: Some(80),
        transition: PowerTransition::Unplugged,
      })
    );
  }

  #[test]
  fn flicker_under_one_second_is_ignored() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 2_000), None);
    assert_eq!(decider.observe(raw(AC_OFFLINE, 0, 50), 2_500), None);
    // The reverted state cleared the pending change; the next plug starts over.
    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 3_000), None);
    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 3_999), None);
    assert!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 4_000).is_some());
  }

  #[test]
  fn percent_change_has_no_transition() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    assert_eq!(decider.observe(raw(AC_OFFLINE, 0, 49), 2_000), None);
    assert_eq!(
      decider.observe(raw(AC_OFFLINE, 0, 49), 3_000),
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

    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 2_000), None);
    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50), 9_000), None);
  }

  #[test]
  fn unknown_percent_is_null() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    assert_eq!(decider.observe(raw(AC_OFFLINE, 0, PERCENT_UNKNOWN), 2_000), None);
    assert_eq!(
      decider.observe(raw(AC_OFFLINE, 0, PERCENT_UNKNOWN), 3_000),
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

    assert_eq!(decider.observe(raw(255, 0, 49), 2_000), None);
    assert_eq!(decider.observe(raw(255, 0, 49), 9_000), None);
    // The next valid reading is a fresh baseline, so it is also silent.
    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 49), 10_000), None);
    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 49), 11_000), None);
  }

  #[test]
  fn no_battery_never_emits() {
    let no_battery = BATTERY_FLAG_NO_BATTERY;
    let mut decider = PowerDecider::default();
    assert_eq!(decider.observe(raw(AC_ONLINE, no_battery, 100), 0), None);

    // Even after a valid baseline, losing the battery resets and stays silent.
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));
    assert_eq!(decider.observe(raw(AC_ONLINE, no_battery, 50), 2_000), None);
    assert_eq!(decider.observe(raw(AC_ONLINE, no_battery, 49), 9_000), None);
  }

  #[test]
  fn both_ac_and_percent_change_prefer_the_transition() {
    let mut decider = PowerDecider::default();
    initial(&mut decider, raw(AC_OFFLINE, 0, 50));

    assert_eq!(decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 51), 2_000), None);
    assert_eq!(
      decider.observe(raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 51), 3_000),
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
      pending: None,
    };
    let status = raw(AC_ONLINE, BATTERY_FLAG_CHARGING, 50);
    assert_eq!(decide(state, status, 1_000), decide(state, status, 1_000));
  }
}
