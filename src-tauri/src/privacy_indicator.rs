//! Mic/camera usage indicator (Phase 5 follow-up).
//!
//! Windows records *when* an app last used the microphone or webcam in HKCU.
//! This module only reads those usage timestamps. It never opens an audio or
//! video device, never captures media, and never sends anything off this PC.
//!
//! The registry read and poll thread are Windows-only. The snapshot/detection
//! logic below is pure so it can be unit-tested on every platform.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Registry poll cadence. The first reading is a silent baseline.
pub const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1_000);
/// One app/device pair may raise an event at most once in this window.
pub const COOLDOWN_MS: u64 = 30_000;
/// Wait one poll after a start so a camera start within one second can join it.
pub const COMBINE_BUFFER_MS: u64 = 1_000;
/// FILETIME ticks in one second (100 ns per tick).
const FILETIME_SECOND: u64 = 10_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
  Microphone,
  Camera,
}

impl DeviceKind {
  fn rank(self) -> u8 {
    match self {
      DeviceKind::Microphone => 0,
      DeviceKind::Camera => 1,
    }
  }

  #[cfg(windows)]
  fn registry_path(self) -> &'static str {
    match self {
      DeviceKind::Microphone => r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone",
      DeviceKind::Camera => r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam",
    }
  }
}

/// A registry subkey, scoped by device kind so the same package can have a mic
/// start and a camera start without overwriting itself in the snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsageKey {
  pub kind: DeviceKind,
  pub subkey: String,
}

/// `LastUsedTimeStart` / `LastUsedTimeStop` as raw FILETIME QWORDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageTimes {
  pub start: u64,
  pub stop: u64,
}

pub type Snapshot = HashMap<UsageKey, UsageTimes>;

/// A newly detected device start. `start` is the raw FILETIME QWORD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
  pub kind: DeviceKind,
  pub subkey: String,
  pub start: u64,
}

/// Rust → frontend `privacy-state` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyEvent {
  pub kinds: Vec<DeviceKind>,
  pub app: String,
}

/**
 * Pure start detector.
 *
 * A start fires only when this subkey's `LastUsedTimeStart` is new or changed
 * and `start > stop` (`stop == 0` means Windows still considers it in use). A
 * stop or an unchanged stale value never fires. The startup snapshot is kept
 * silent by `PrivacyTracker`, which owns the baseline.
 */
pub fn detect_starts(prev: &Snapshot, curr: &Snapshot) -> Vec<Start> {
  let mut starts: Vec<Start> = curr
    .iter()
    .filter_map(|(key, times)| {
      if times.start <= times.stop {
        return None;
      }
      let changed = match prev.get(key) {
        None => true,
        Some(before) => before.start != times.start,
      };
      changed.then(|| Start {
        kind: key.kind,
        subkey: key.subkey.clone(),
        start: times.start,
      })
    })
    .collect();

  starts.sort_by(|a, b| {
    a.kind
      .rank()
      .cmp(&b.kind.rank())
      .then_with(|| a.subkey.cmp(&b.subkey))
  });
  starts
}

/// Friendly app name from a raw ConsentStore subkey.
///
/// Desktop apps live under `NonPackaged` and are stored as an exe path whose
/// `\` separators were replaced by `#`. Packaged apps use a package family
/// name such as `Microsoft.WindowsCamera_8wekyb3d8bbwe`.
pub fn friendly_app_name(subkey: &str) -> String {
  let desktop = subkey.contains('#');
  let raw = if desktop {
    subkey.rsplit(['#', '/', '\\']).next().unwrap_or(subkey)
  } else {
    subkey.split('_').next().unwrap_or(subkey)
  };
  // Desktop keys end in `exe` (even if the stored name has no `#` separators);
  // drop that extension. Packaged keys put the readable name after the last
  // dot (before the publisher underscore).
  let base = if raw.to_ascii_lowercase().ends_with(".exe") {
    &raw[..raw.len() - 4]
  } else if desktop {
    raw
  } else {
    raw.rsplit_once('.').map_or(raw, |(_, tail)| tail)
  };
  if base.is_empty() {
    return "App".into();
  }

  let mapped = match base.to_ascii_lowercase().as_str() {
    "chrome" => "Chrome",
    "msedge" => "Edge",
    "firefox" => "Firefox",
    "teams" | "ms-teams" => "Teams",
    "zoom" => "Zoom",
    "discord" => "Discord",
    _ => return capitalize(base),
  };
  mapped.into()
}

fn capitalize(value: &str) -> String {
  let mut chars = value.chars();
  match chars.next() {
    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    None => String::new(),
  }
}

fn combined_app_name(first: &str, second: &str) -> String {
  if first.eq_ignore_ascii_case(second) {
    first.to_string()
  } else {
    format!("{first} & {second}")
  }
}

#[derive(Debug, Clone)]
struct PendingStart {
  start: Start,
  app: String,
  detected_at_ms: u64,
}

/// Converts raw starts into cooldown-filtered, possibly combined UI events.
#[derive(Debug, Default)]
pub struct PrivacyTracker {
  has_baseline: bool,
  prev: Snapshot,
  pending: Option<PendingStart>,
  last_emitted: HashMap<(DeviceKind, String), u64>,
}

impl PrivacyTracker {
  pub fn observe(&mut self, curr: Snapshot, now_ms: u64) -> Vec<PrivacyEvent> {
    if !self.has_baseline {
      self.has_baseline = true;
      self.prev = curr;
      return Vec::new();
    }

    let starts = detect_starts(&self.prev, &curr);
    self.prev = curr;

    let mut events = Vec::new();
    for start in starts {
      let app = friendly_app_name(&start.subkey);
      if self.is_cooling(start.kind, &app, now_ms) {
        continue;
      }

      if let Some(pending) = self.pending.take() {
        let can_combine = pending.start.kind != start.kind
          && pending.start.start.abs_diff(start.start) <= FILETIME_SECOND
          && now_ms.saturating_sub(pending.detected_at_ms) <= COMBINE_BUFFER_MS;
        if can_combine {
          events.push(self.emit_combined(pending, start, app, now_ms));
          continue;
        }

        // Different start: the buffered one must still be shown.
        if let Some(event) = self.emit_pending(pending, now_ms) {
          events.push(event);
        }
        if self.is_cooling(start.kind, &app, now_ms) {
          continue;
        }
      }

      // A restart of the same app/device before the buffer flushes is one
      // event, not two. Keep the newest FILETIME and restart the buffer.
      if let Some(existing) = self.pending.as_mut() {
        if existing.start.kind == start.kind && existing.app.eq_ignore_ascii_case(&app) {
          existing.start = start;
          existing.app = app;
          existing.detected_at_ms = now_ms;
          continue;
        }
      }

      self.pending = Some(PendingStart {
        start,
        app,
        detected_at_ms: now_ms,
      });
    }

    if let Some(pending) = self.pending.take() {
      if now_ms.saturating_sub(pending.detected_at_ms) >= COMBINE_BUFFER_MS {
        if let Some(event) = self.emit_pending(pending, now_ms) {
          events.push(event);
        }
      } else {
        self.pending = Some(pending);
      }
    }

    events
  }

  fn is_cooling(&self, kind: DeviceKind, app: &str, now_ms: u64) -> bool {
    self
      .last_emitted
      .get(&(kind, app.to_ascii_lowercase()))
      .is_some_and(|at| now_ms.saturating_sub(*at) < COOLDOWN_MS)
  }

  fn emit_pending(&mut self, pending: PendingStart, now_ms: u64) -> Option<PrivacyEvent> {
    self.last_emitted.insert(
      (pending.start.kind, pending.app.to_ascii_lowercase()),
      now_ms,
    );
    Some(PrivacyEvent {
      kinds: vec![pending.start.kind],
      app: pending.app,
    })
  }

  fn emit_combined(
    &mut self,
    pending: PendingStart,
    start: Start,
    app: String,
    now_ms: u64,
  ) -> PrivacyEvent {
    let mut kinds = vec![pending.start.kind, start.kind];
    kinds.sort_by_key(|kind| kind.rank());
    let name = combined_app_name(&pending.app, &app);

    // Cool down the individual app/kind pairs too, even when the two apps
    // differ and the event's `app` label is combined.
    for (kind, owner) in [(pending.start.kind, pending.app.as_str()), (start.kind, app.as_str())] {
      self
        .last_emitted
        .insert((kind, owner.to_ascii_lowercase()), now_ms);
    }
    PrivacyEvent { kinds, app: name }
  }
}

// ---------------------------------------------------------------------------
// Windows-only registry access. Read-only HKCU, no admin rights, no media APIs.
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod registry {
  use super::*;
  use windows::Win32::Foundation::{
    ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS,
  };
  use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, REG_QWORD, REG_VALUE_TYPE, RegCloseKey, RegEnumKeyExW,
    RegOpenKeyExW, RegQueryValueExW,
  };
  use windows::core::{PCWSTR, PWSTR};

  const START_VALUE: &str = "LastUsedTimeStart";
  const STOP_VALUE: &str = "LastUsedTimeStop";
  const NONPACKAGED: &str = "NonPackaged";

  struct OwnedKey(HKEY);

  impl OwnedKey {
    fn raw(&self) -> HKEY {
      self.0
    }
  }

  impl Drop for OwnedKey {
    fn drop(&mut self) {
      if !self.0 .0.is_null() {
        unsafe {
          let _ = RegCloseKey(self.0);
        }
      }
    }
  }

  fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
  }

  fn open_key(base: HKEY, path: &str) -> Option<OwnedKey> {
    let path = wide(path);
    let mut handle = HKEY(std::ptr::null_mut());
    let status = unsafe {
      RegOpenKeyExW(
        base,
        PCWSTR(path.as_ptr()),
        None,
        KEY_READ,
        &mut handle,
      )
    };
    if status.0 == ERROR_SUCCESS.0 {
      Some(OwnedKey(handle))
    } else {
      None
    }
  }

  fn enumerate_subkeys(key: HKEY) -> Vec<String> {
    let mut names = Vec::new();
    let mut capacity = 512usize;

    loop {
      let mut buffer = vec![0u16; capacity];
      let mut length = capacity as u32;
      let index = names.len() as u32;
      let status = unsafe {
        RegEnumKeyExW(
          key,
          index,
          Some(PWSTR(buffer.as_mut_ptr())),
          &mut length,
          None,
          None,
          None,
          None,
        )
      };

      if status.0 == ERROR_NO_MORE_ITEMS.0 {
        break;
      }
      if status.0 == ERROR_MORE_DATA.0 && capacity < 32_768 {
        capacity *= 2;
        continue;
      }
      if status.0 != ERROR_SUCCESS.0 {
        break;
      }
      names.push(String::from_utf16_lossy(&buffer[..length as usize]));
    }

    names
  }

  fn read_qword(key: HKEY, value_name: &str) -> Option<u64> {
    let value_name = wide(value_name);
    let mut value_type = REG_VALUE_TYPE(0);
    let mut bytes = [0u8; 8];
    let mut size = 8u32;
    let status = unsafe {
      RegQueryValueExW(
        key,
        PCWSTR(value_name.as_ptr()),
        None,
        Some(&mut value_type),
        Some(bytes.as_mut_ptr()),
        Some(&mut size),
      )
    };
    if status.0 == ERROR_SUCCESS.0 && value_type.0 == REG_QWORD.0 && size == 8 {
      Some(u64::from_le_bytes(bytes))
    } else {
      None
    }
  }

  fn add_app(
    snapshot: &mut Snapshot,
    kind: DeviceKind,
    subkey: &str,
    key: HKEY,
  ) {
    let (Some(start), Some(stop)) = (read_qword(key, START_VALUE), read_qword(key, STOP_VALUE))
    else {
      return;
    };
    snapshot.insert(
      UsageKey {
        kind,
        subkey: subkey.to_string(),
      },
      UsageTimes { start, stop },
    );
  }

  fn read_kind(kind: DeviceKind, debug: bool) -> Snapshot {
    let mut snapshot = Snapshot::new();
    let Some(root) = open_key(HKEY_CURRENT_USER, kind.registry_path()) else {
      return snapshot;
    };

    for name in enumerate_subkeys(root.raw()) {
      if debug {
        log::info!("[privacy] raw {kind:?} subkey: {name}");
      }
      if name.eq_ignore_ascii_case(NONPACKAGED) {
        let Some(nonpackaged) = open_key(root.raw(), &name) else {
          continue;
        };
        for exe in enumerate_subkeys(nonpackaged.raw()) {
          if debug {
            log::info!("[privacy] raw {kind:?} NonPackaged subkey: {exe}");
          }
          if let Some(app) = open_key(nonpackaged.raw(), &exe) {
            add_app(&mut snapshot, kind, &exe, app.raw());
          }
        }
      } else if let Some(app) = open_key(root.raw(), &name) {
        add_app(&mut snapshot, kind, &name, app.raw());
      }
    }

    snapshot
  }

  pub(super) fn read_snapshot(debug: bool) -> Snapshot {
    let mut snapshot = read_kind(DeviceKind::Microphone, debug);
    snapshot.extend(read_kind(DeviceKind::Camera, debug));
    snapshot
  }
}

/// Poll HKCU usage records every second on its own thread. Startup is silent.
#[cfg(windows)]
pub fn start(app: tauri::AppHandle) {
  use tauri::Emitter;

  let debug = crate::music::debug_enabled();
  std::thread::Builder::new()
    .name("float-privacy".into())
    .spawn(move || {
      let mut tracker = PrivacyTracker::default();
      let started = std::time::Instant::now();

      loop {
        let snapshot = registry::read_snapshot(debug);
        let now_ms = started.elapsed().as_millis() as u64;
        for event in tracker.observe(snapshot, now_ms) {
          if debug {
            log::info!("[privacy] emit {:?} app={}", event.kinds, event.app);
          }
          if let Err(err) = app.emit("privacy-state", event) {
            log::warn!("failed to emit privacy-state: {err}");
          }
        }
        std::thread::sleep(POLL_INTERVAL);
      }
    })
    .expect("failed to spawn privacy indicator thread");
}

#[cfg(not(windows))]
pub fn start(_app: tauri::AppHandle) {}

#[cfg(test)]
mod tests {
  use super::*;

  fn key(kind: DeviceKind, subkey: &str) -> UsageKey {
    UsageKey {
      kind,
      subkey: subkey.into(),
    }
  }

  fn times(start: u64, stop: u64) -> UsageTimes {
    UsageTimes { start, stop }
  }

  fn snapshot(entries: Vec<(DeviceKind, &str, u64, u64)>) -> Snapshot {
    entries
      .into_iter()
      .map(|(kind, subkey, start, stop)| (key(kind, subkey), times(start, stop)))
      .collect()
  }

  fn starts(events: Vec<Start>) -> Vec<(DeviceKind, String)> {
    events
      .into_iter()
      .map(|start| (start.kind, start.subkey))
      .collect()
  }

  #[test]
  fn new_start_fires() {
    let prev = snapshot(vec![(DeviceKind::Microphone, "OLD_APP", 10, 20)]);
    let curr = snapshot(vec![
      (DeviceKind::Microphone, "OLD_APP", 10, 20),
      (DeviceKind::Microphone, "NEW_APP", 100, 0),
    ]);

    assert_eq!(
      starts(detect_starts(&prev, &curr)),
      vec![(DeviceKind::Microphone, "NEW_APP".into())]
    );
  }

  #[test]
  fn changed_start_fires() {
    let prev = snapshot(vec![(DeviceKind::Camera, "APP", 100, 0)]);
    let curr = snapshot(vec![(DeviceKind::Camera, "APP", 200, 0)]);

    assert_eq!(
      starts(detect_starts(&prev, &curr)),
      vec![(DeviceKind::Camera, "APP".into())]
    );
  }

  #[test]
  fn unchanged_start_does_not_fire() {
    let prev = snapshot(vec![(DeviceKind::Microphone, "APP", 100, 0)]);
    let curr = prev.clone();

    assert!(detect_starts(&prev, &curr).is_empty());
  }

  #[test]
  fn a_stop_does_not_fire() {
    let prev = snapshot(vec![(DeviceKind::Microphone, "APP", 100, 0)]);
    let curr = snapshot(vec![(DeviceKind::Microphone, "APP", 100, 200)]);

    assert!(detect_starts(&prev, &curr).is_empty());
  }

  #[test]
  fn stale_active_value_in_the_baseline_does_not_fire() {
    // A crash can leave start > stop behind. The startup snapshot must not
    // report it as a new use, and neither may a later unchanged poll.
    let prev = snapshot(vec![(DeviceKind::Microphone, "STALE", 100, 0)]);
    let curr = prev.clone();

    assert!(detect_starts(&prev, &curr).is_empty());
  }

  #[test]
  fn two_apps_fire_together() {
    let prev = snapshot(vec![(DeviceKind::Microphone, "OLD_APP", 10, 20)]);
    let curr = snapshot(vec![
      (DeviceKind::Microphone, "OLD_APP", 10, 20),
      (DeviceKind::Microphone, "APP_A", 100, 0),
      (DeviceKind::Microphone, "APP_B", 110, 0),
    ]);

    assert_eq!(
      starts(detect_starts(&prev, &curr)),
      vec![
        (DeviceKind::Microphone, "APP_A".into()),
        (DeviceKind::Microphone, "APP_B".into()),
      ]
    );
  }

  #[test]
  fn microphone_and_camera_are_tracked_separately() {
    let prev = snapshot(vec![(DeviceKind::Microphone, "SAME_PACKAGE", 10, 20)]);
    let curr = snapshot(vec![
      (DeviceKind::Microphone, "SAME_PACKAGE", 100, 0),
      (DeviceKind::Camera, "SAME_PACKAGE", 100, 0),
    ]);

    assert_eq!(
      starts(detect_starts(&prev, &curr)),
      vec![
        (DeviceKind::Microphone, "SAME_PACKAGE".into()),
        (DeviceKind::Camera, "SAME_PACKAGE".into()),
      ]
    );
  }

  #[test]
  fn friendly_names_cover_desktop_and_packaged_apps() {
    assert_eq!(friendly_app_name(r"C:#Program Files#Google#Chrome#Application#chrome.exe"), "Chrome");
    assert_eq!(friendly_app_name("chrome.exe"), "Chrome");
    assert_eq!(friendly_app_name(r"C:#Program Files (x86)#Microsoft#Edge#Application#msedge.exe"), "Edge");
    assert_eq!(friendly_app_name(r"C:#Program Files#Mozilla Firefox#firefox.exe"), "Firefox");
    assert_eq!(friendly_app_name(r"C:#Program Files#Microsoft#Teams#ms-teams.exe"), "Teams");
    assert_eq!(friendly_app_name(r"C:#Program Files#zoom#Zoom.exe"), "Zoom");
    assert_eq!(friendly_app_name(r"C:#Program Files#Discord#discord.exe"), "Discord");
    assert_eq!(friendly_app_name(r"C:#Program Files#Spotify#spotify.exe"), "Spotify");
    assert_eq!(
      friendly_app_name("Microsoft.WindowsCamera_8wekyb3d8bbwe"),
      "WindowsCamera"
    );
    assert_eq!(
      friendly_app_name("SpotifyAB.SpotifyMusic_zpdnekdrzrea0"),
      "SpotifyMusic"
    );
  }

  #[test]
  fn first_poll_is_a_silent_baseline() {
    let mut tracker = PrivacyTracker::default();
    let active = snapshot(vec![(DeviceKind::Microphone, "APP", 100, 0)]);

    assert!(tracker.observe(active.clone(), 0).is_empty());
    assert!(tracker.observe(active, 1_000).is_empty());
  }

  #[test]
  fn close_mic_and_camera_starts_are_combined() {
    let mut tracker = PrivacyTracker::default();
    tracker.observe(Snapshot::new(), 0);

    let mic = snapshot(vec![(DeviceKind::Microphone, "APP", 1_000, 0)]);
    assert!(tracker.observe(mic, 1_000).is_empty());

    let both = snapshot(vec![
      (DeviceKind::Microphone, "APP", 1_000, 0),
      (DeviceKind::Camera, "APP", 9_000_000, 0),
    ]);
    let events = tracker.observe(both, 2_000);

    assert_eq!(
      events,
      vec![PrivacyEvent {
        kinds: vec![DeviceKind::Microphone, DeviceKind::Camera],
        app: "APP".into(),
      }]
    );
  }

  #[test]
  fn distant_starts_stay_separate() {
    let mut tracker = PrivacyTracker::default();
    tracker.observe(Snapshot::new(), 0);

    let mic = snapshot(vec![(DeviceKind::Microphone, "APP", 1_000, 0)]);
    assert!(tracker.observe(mic, 1_000).is_empty());
    // Flush the buffered mic start.
    assert_eq!(
      tracker.observe(snapshot(vec![(DeviceKind::Microphone, "APP", 1_000, 0)]), 2_000),
      vec![PrivacyEvent {
        kinds: vec![DeviceKind::Microphone],
        app: "APP".into(),
      }]
    );

    let both = snapshot(vec![
      (DeviceKind::Microphone, "APP", 1_000, 0),
      (DeviceKind::Camera, "APP", 30_000 * 10_000, 0),
    ]);
    assert!(tracker.observe(both, 3_000).is_empty());
    assert_eq!(
      tracker.observe(snapshot(vec![
        (DeviceKind::Microphone, "APP", 1_000, 0),
        (DeviceKind::Camera, "APP", 30_000 * 10_000, 0),
      ]), 4_000),
      vec![PrivacyEvent {
        kinds: vec![DeviceKind::Camera],
        app: "APP".into(),
      }]
    );
  }

  #[test]
  fn same_app_and_kind_is_cooled_down_for_thirty_seconds() {
    let mut tracker = PrivacyTracker::default();
    tracker.observe(Snapshot::new(), 0);

    let first = snapshot(vec![(DeviceKind::Microphone, "APP", 100, 0)]);
    assert!(tracker.observe(first.clone(), 1_000).is_empty());
    assert_eq!(
      tracker.observe(first, 2_000),
      vec![PrivacyEvent {
        kinds: vec![DeviceKind::Microphone],
        app: "APP".into(),
      }]
    );

    // The start value changes while the cooldown is active: no second event.
    let restarted = snapshot(vec![(DeviceKind::Microphone, "APP", 200, 0)]);
    assert!(tracker.observe(restarted.clone(), 10_000).is_empty());
    assert!(tracker.observe(restarted, 31_999).is_empty());

    // After 30 s the same app/kind may fire again.
    let after_cooldown = snapshot(vec![(DeviceKind::Microphone, "APP", 300, 0)]);
    assert!(tracker.observe(after_cooldown.clone(), 32_000).is_empty());
    assert_eq!(
      tracker.observe(after_cooldown, 33_000),
      vec![PrivacyEvent {
        kinds: vec![DeviceKind::Microphone],
        app: "APP".into(),
      }]
    );
  }
}
