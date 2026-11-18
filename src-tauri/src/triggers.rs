//! Trigger detection (Phase 2).
//!
//! Hot-zone geometry and dwell timing are pure functions so they can be
//! unit-tested; the cursor poll itself is Windows-only.

// The pure helpers are only exercised by tests on non-Windows builds.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::time::{Duration, Instant};

/// Zone width in logical px (design value, scaled to physical px).
pub const ZONE_WIDTH_LOGICAL_PX: f64 = 240.0;
/// Zone height in logical px (design value, scaled to physical px).
pub const ZONE_HEIGHT_LOGICAL_PX: f64 = 4.0;
/// Continuous dwell required before opening, in milliseconds.
pub const DWELL_MS: u64 = 180;

/// One keyboard event as seen by the double-Alt tracker.
pub enum KeyEvent {
  AltDown,
  AltUp,
  OtherDown,
}

pub struct AltTapTracker {
  alt_down: bool,
  clean: bool,
  pressed_at: Option<Instant>,
  last_tap_at: Option<Instant>,
  window: Duration,
}

impl AltTapTracker {
  pub fn new(window_ms: u64) -> Self {
    Self {
      alt_down: false,
      clean: true,
      pressed_at: None,
      last_tap_at: None,
      window: Duration::from_millis(window_ms),
    }
  }
  /// Returns true when a double-tap completes.
  pub fn on_event(&mut self, ev: KeyEvent, now: Instant) -> bool {
    match ev {
      KeyEvent::AltDown => {
        if !self.alt_down {
          self.alt_down = true;
          self.clean = true;
          self.pressed_at = Some(now);
        }
        false
      }
      KeyEvent::OtherDown => {
        self.clean = false;
        self.last_tap_at = None;
        false
      }
      KeyEvent::AltUp => {
        if !self.alt_down {
          return false;
        }
        self.alt_down = false;
        let pressed = self.pressed_at.take().unwrap_or(now);
        if !self.clean {
          self.last_tap_at = None;
          return false;
        }
        match self.last_tap_at {
          Some(prev) if pressed.duration_since(prev) <= self.window => {
            self.last_tap_at = None;
            true
          }
          _ => {
            self.last_tap_at = Some(pressed);
            false
          }
        }
      }
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
  pub left: f64,
  pub top: f64,
  pub right: f64,
  pub bottom: f64,
}

/// Zone centered horizontally on a monitor, anchored to its top edge.
/// All values are physical px (`scale_factor` converts the logical design size).
pub fn zone_rect(
  monitor_left: f64,
  monitor_top: f64,
  monitor_width: f64,
  scale_factor: f64,
) -> Rect {
  let width = ZONE_WIDTH_LOGICAL_PX * scale_factor;
  let height = ZONE_HEIGHT_LOGICAL_PX * scale_factor;
  let center_x = monitor_left + monitor_width / 2.0;
  Rect {
    left: center_x - width / 2.0,
    top: monitor_top,
    right: center_x + width / 2.0,
    bottom: monitor_top + height,
  }
}

pub fn contains(rect: &Rect, x: f64, y: f64) -> bool {
  x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

/// True when the hot zone must not fire, regardless of dwell.
pub fn suppressed(mouse_down: bool, foreground_fullscreen: bool, hide_in_fullscreen: bool) -> bool {
  mouse_down || (hide_in_fullscreen && foreground_fullscreen)
}

/// Fires once per continuous dwell inside the zone.
#[derive(Debug, Default)]
pub struct DwellTracker {
  inside_since: Option<u64>,
  fired: bool,
}

impl DwellTracker {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn reset(&mut self) {
    self.inside_since = None;
    self.fired = false;
  }

  /// Feed one poll. Returns `true` on the first poll at or after `DWELL_MS`.
  pub fn update(&mut self, inside: bool, now_ms: u64) -> bool {
    if !inside {
      self.reset();
      return false;
    }
    let since = *self.inside_since.get_or_insert(now_ms);
    if !self.fired && now_ms.saturating_sub(since) >= DWELL_MS {
      self.fired = true;
      return true;
    }
    false
  }
}

#[cfg(target_os = "windows")]
mod imp {
  use super::*;
  use std::time::{Duration, Instant};
  use tauri::Emitter;
  use windows::Win32::Foundation::{HWND, POINT, RECT};
  use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
  };
  use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
  use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_XBUTTON1, VK_XBUTTON2,
  };
  use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetForegroundWindow, GetWindowRect};

  const POLL_INTERVAL: Duration = Duration::from_millis(16); // ~60 Hz

  struct MonitorGeometry {
    left: f64,
    top: f64,
    width: f64,
    scale_factor: f64,
  }

  fn monitor_rect(hmonitor: HMONITOR) -> Option<RECT> {
    let mut info = MONITORINFO {
      cbSize: std::mem::size_of::<MONITORINFO>() as u32,
      ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(hmonitor, &mut info) }.as_bool() {
      return None;
    }
    Some(info.rcMonitor)
  }

  fn monitor_geometry(hmonitor: HMONITOR) -> Option<MonitorGeometry> {
    let rect = monitor_rect(hmonitor)?;
    let mut dpi_x = 96u32;
    let mut dpi_y = 96u32;
    let scale_factor =
      if unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.is_ok() {
        dpi_x as f64 / 96.0
      } else {
        1.0
      };
    Some(MonitorGeometry {
      left: rect.left as f64,
      top: rect.top as f64,
      width: (rect.right - rect.left) as f64,
      scale_factor,
    })
  }

  fn cursor_in_zone() -> bool {
    unsafe {
      let mut point = POINT::default();
      if GetCursorPos(&mut point).is_err() {
        return false;
      }
      let hmonitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
      let Some(monitor) = monitor_geometry(hmonitor) else {
        return false;
      };
      let zone = zone_rect(
        monitor.left,
        monitor.top,
        monitor.width,
        monitor.scale_factor,
      );
      contains(&zone, point.x as f64, point.y as f64)
    }
  }

  fn mouse_button_down() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
      .iter()
      .any(|vk| (unsafe { GetAsyncKeyState(vk.0 as i32) } as u16) & 0x8000 != 0)
  }

  /// Foreground window covers its monitor's full bounds (heuristic; maximized
  /// windows normally stop at the work area).
  fn foreground_fullscreen() -> bool {
    unsafe {
      let hwnd: HWND = GetForegroundWindow();
      if hwnd.0.is_null() {
        return false;
      }
      let mut window = RECT::default();
      if GetWindowRect(hwnd, &mut window).is_err() {
        return false;
      }
      let Some(monitor) = monitor_rect(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)) else {
        return false;
      };
      window.left <= monitor.left
        && window.top <= monitor.top
        && window.right >= monitor.right
        && window.bottom >= monitor.bottom
    }
  }

  pub fn start(app: tauri::AppHandle) {
    let (top_edge, hide_in_fullscreen) = match crate::settings::load_settings(&app) {
      Ok(settings) => (settings.notch.top_edge, settings.notch.hide_in_fullscreen),
      Err(_) => (true, true),
    };
    if !top_edge {
      log::info!("hot zone disabled by settings");
      return;
    }

    let spawned = std::thread::Builder::new()
      .name("float-hot-zone".into())
      .spawn(move || {
        let mut tracker = DwellTracker::new();
        let started = Instant::now();
        loop {
          let now_ms = started.elapsed().as_millis() as u64;
          let inside = cursor_in_zone();
          let blocked = suppressed(
            mouse_button_down(),
            foreground_fullscreen(),
            hide_in_fullscreen,
          );
          if blocked {
            tracker.reset();
          } else if tracker.update(inside, now_ms) {
            log::info!("hot zone dwell reached, emitting open-request");
            if let Err(err) = app.emit("open-request", ()) {
              log::warn!("failed to emit open-request: {err}");
            }
          }
          std::thread::sleep(POLL_INTERVAL);
        }
      });

    if let Err(err) = spawned {
      log::error!("failed to start hot zone thread: {err}");
    }
  }
}

#[cfg(target_os = "windows")]
pub use imp::start;

/// Double-tap window in ms (design value, docs/FEATURES.md).
pub const ALT_TAP_WINDOW_MS: u64 = 350;

#[cfg(target_os = "windows")]
mod alt_hook {
  use super::{AltTapTracker, KeyEvent, ALT_TAP_WINDOW_MS};
  use std::cell::RefCell;
  use std::time::Instant;
  use tauri::Emitter;
  use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
  use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
  };

  const VK_MENU: u32 = 0x12;
  const VK_LMENU: u32 = 0xA4;
  const VK_RMENU: u32 = 0xA5;

  struct HookState {
    tracker: AltTapTracker,
    app: tauri::AppHandle,
  }

  thread_local! {
    /// The hook is called on the thread that installed it, so a thread-local
    /// keeps the callback lock-free.
    static STATE: RefCell<Option<HookState>> = const { RefCell::new(None) };
  }

  fn is_alt(vk: u32) -> bool {
    vk == VK_MENU || vk == VK_LMENU || vk == VK_RMENU
  }

  /// `WH_KEYBOARD_LL` callback. Every event is passed on untouched; only the
  /// tracker state is touched here.
  unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
      // SAFETY: for `WH_KEYBOARD_LL` with `code >= 0` the system passes a
      // KBDLLHOOKSTRUCT pointer.
      let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
      // Injected keys (Alt-menu suppression, other apps' macros) never count.
      if !info.flags.contains(LLKHF_INJECTED) {
        let message = wparam.0 as u32;
        let down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
        let up = message == WM_KEYUP || message == WM_SYSKEYUP;
        if down || up {
          let alt = is_alt(info.vkCode);
          let event = match (alt, down) {
            (true, true) => Some(KeyEvent::AltDown),
            (true, false) => Some(KeyEvent::AltUp),
            (false, true) => Some(KeyEvent::OtherDown),
            (false, false) => None,
          };
          if let Some(event) = event {
            if alt {
              log::info!(
                "alt {} (vk 0x{:X})",
                if down { "down" } else { "up" },
                info.vkCode
              );
            }
            let fire = STATE.with(|cell| {
              let mut slot = cell.borrow_mut();
              let state = slot.as_mut()?;
              state
                .tracker
                .on_event(event, Instant::now())
                .then(|| state.app.clone())
            });
            if let Some(app) = fire {
              log::info!("double alt detected, emitting toggle-request");
              if let Err(err) = app.emit("toggle-request", serde_json::json!({ "source": "alt" })) {
                log::warn!("failed to emit toggle-request: {err}");
              }
            }
          }
        }
      }
    }
    // SAFETY: plain pass-through; the hook never swallows a key.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
  }

  /// Installs the low-level keyboard hook on its own thread. The thread must
  /// run a message loop or the hook is never called.
  pub fn start(app: tauri::AppHandle) {
    let enabled = match crate::settings::load_settings(&app) {
      Ok(settings) => settings.notch.alt_alt,
      Err(err) => {
        log::warn!("failed to read altAlt setting ({err}), keeping Alt Alt enabled");
        true
      }
    };
    if !enabled {
      log::info!("alt alt disabled by settings");
      return;
    }

    let spawned = std::thread::Builder::new()
      .name("float-alt-hook".into())
      .spawn(move || {
        STATE.with(|cell| {
          *cell.borrow_mut() = Some(HookState {
            tracker: AltTapTracker::new(ALT_TAP_WINDOW_MS),
            app,
          });
        });

        // SAFETY: thread-local state is in place; hMod is NULL for a
        // low-level hook, which also ties the hook to this thread.
        let hook = match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0) } {
          Ok(hook) => hook,
          Err(err) => {
            log::error!("failed to install keyboard hook: {err}");
            return;
          }
        };
        log::info!("keyboard hook installed (double alt, {ALT_TAP_WINDOW_MS} ms window)");

        let mut message = MSG::default();
        loop {
          // SAFETY: blocking wait; the low-level hook runs while we wait here,
          // so messages are intentionally not dispatched.
          let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
          if result.0 == 0 {
            break;
          }
          if result.0 == -1 {
            log::error!("keyboard hook message loop failed");
            break;
          }
        }

        // SAFETY: `hook` came from SetWindowsHookExW on this thread.
        let _ = unsafe { UnhookWindowsHookEx(hook) };
        STATE.with(|cell| *cell.borrow_mut() = None);
        log::info!("keyboard hook removed");
      });

    if let Err(err) = spawned {
      log::error!("failed to start alt hook thread: {err}");
    }
  }
}

#[cfg(target_os = "windows")]
pub use alt_hook::start as start_alt_hook;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn zone_is_centered_on_monitor() {
    let zone = zone_rect(0.0, 0.0, 1920.0, 1.0);
    assert_eq!(zone.left, 840.0);
    assert_eq!(zone.right, 1080.0);
    assert_eq!(zone.top, 0.0);
    assert_eq!(zone.bottom, 4.0);
  }

  #[test]
  fn zone_scales_with_dpi_and_offsets_with_monitor() {
    let zone = zone_rect(1920.0, -180.0, 2560.0, 1.5);
    assert_eq!(zone.left, 1920.0 + 1280.0 - 180.0);
    assert_eq!(zone.right, 1920.0 + 1280.0 + 180.0);
    assert_eq!(zone.top, -180.0);
    assert_eq!(zone.bottom, -174.0);
  }

  #[test]
  fn zone_edges_are_half_open() {
    let zone = zone_rect(0.0, 0.0, 1920.0, 1.0);
    assert!(contains(&zone, 840.0, 0.0));
    assert!(contains(&zone, 1079.9, 3.9));
    assert!(!contains(&zone, 839.9, 0.0));
    assert!(!contains(&zone, 1080.0, 0.0));
    assert!(!contains(&zone, 960.0, 4.0));
  }

  #[test]
  fn dwell_requires_180_ms() {
    let mut tracker = DwellTracker::new();
    assert!(!tracker.update(true, 0));
    assert!(!tracker.update(true, 179));
    assert!(tracker.update(true, 180));
  }

  #[test]
  fn dwell_fires_once_per_entry() {
    let mut tracker = DwellTracker::new();
    assert!(!tracker.update(true, 0));
    assert!(tracker.update(true, 200));
    assert!(!tracker.update(true, 400));
    assert!(!tracker.update(true, 600));
  }

  #[test]
  fn leaving_zone_rearms_dwell() {
    let mut tracker = DwellTracker::new();
    assert!(!tracker.update(true, 0));
    assert!(tracker.update(true, 180));
    assert!(!tracker.update(false, 200));
    assert!(!tracker.update(true, 300));
    assert!(!tracker.update(true, 479));
    assert!(tracker.update(true, 480));
  }

  #[test]
  fn suppression_rules() {
    assert!(suppressed(true, false, true));
    assert!(suppressed(true, false, false));
    assert!(suppressed(false, true, true));
    assert!(!suppressed(false, true, false));
    assert!(!suppressed(false, false, true));
  }

  const WINDOW: u64 = ALT_TAP_WINDOW_MS;

  fn at(base: Instant, ms: u64) -> Instant {
    base + Duration::from_millis(ms)
  }

  /// Press and release Alt at `ms` (held for 20 ms).
  fn tap(tracker: &mut AltTapTracker, base: Instant, ms: u64) -> bool {
    assert!(!tracker.on_event(KeyEvent::AltDown, at(base, ms)));
    tracker.on_event(KeyEvent::AltUp, at(base, ms + 20))
  }

  #[test]
  fn alt_double_tap_within_350_ms_fires() {
    let base = Instant::now();
    let mut tracker = AltTapTracker::new(WINDOW);
    assert!(!tap(&mut tracker, base, 0));
    assert!(tap(&mut tracker, base, 200));
  }

  #[test]
  fn alt_taps_500_ms_apart_do_not_fire() {
    let base = Instant::now();
    let mut tracker = AltTapTracker::new(WINDOW);
    assert!(!tap(&mut tracker, base, 0));
    assert!(!tap(&mut tracker, base, 500));
  }

  #[test]
  fn alt_tab_does_not_fire() {
    let base = Instant::now();
    let mut tracker = AltTapTracker::new(WINDOW);
    assert!(!tracker.on_event(KeyEvent::AltDown, at(base, 0)));
    assert!(!tracker.on_event(KeyEvent::OtherDown, at(base, 10)));
    assert!(!tracker.on_event(KeyEvent::AltUp, at(base, 40)));
    // The dirty tap is discarded, so the next clean tap starts the sequence.
    assert!(!tap(&mut tracker, base, 100));
    assert!(tap(&mut tracker, base, 250));
  }

  #[test]
  fn key_between_taps_resets_sequence() {
    let base = Instant::now();
    let mut tracker = AltTapTracker::new(WINDOW);
    assert!(!tap(&mut tracker, base, 0));
    assert!(!tracker.on_event(KeyEvent::OtherDown, at(base, 60)));
    // Would have fired without the reset (100 ms after the previous tap).
    assert!(!tap(&mut tracker, base, 100));
    assert!(tap(&mut tracker, base, 200));
  }

  #[test]
  fn held_alt_auto_repeat_counts_as_one_press() {
    let base = Instant::now();
    let mut tracker = AltTapTracker::new(WINDOW);
    assert!(!tracker.on_event(KeyEvent::AltDown, at(base, 0)));
    assert!(!tracker.on_event(KeyEvent::AltDown, at(base, 40)));
    assert!(!tracker.on_event(KeyEvent::AltDown, at(base, 80)));
    assert!(!tracker.on_event(KeyEvent::AltUp, at(base, 100)));
    // Timed from the first key-down: 400 ms is outside the window, while the
    // last auto-repeat (80 ms) would have been only 320 ms earlier.
    assert!(!tracker.on_event(KeyEvent::AltDown, at(base, 400)));
    assert!(!tracker.on_event(KeyEvent::AltUp, at(base, 420)));
  }

  #[test]
  fn third_tap_after_fire_starts_new_sequence() {
    let base = Instant::now();
    let mut tracker = AltTapTracker::new(WINDOW);
    assert!(!tap(&mut tracker, base, 0));
    assert!(tap(&mut tracker, base, 100));
    assert!(!tap(&mut tracker, base, 150));
    assert!(tap(&mut tracker, base, 250));
  }

  #[test]
  fn alt_up_without_alt_down_is_ignored() {
    let base = Instant::now();
    let mut tracker = AltTapTracker::new(WINDOW);
    assert!(!tracker.on_event(KeyEvent::AltUp, at(base, 0)));
    assert!(!tracker.on_event(KeyEvent::AltUp, at(base, 10)));
  }
}
