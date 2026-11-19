//! Trigger detection (Phase 2).
//!
//! Hot-zone geometry and dwell timing are pure functions so they can be
//! unit-tested; the cursor poll itself is Windows-only.

// The pure helpers are only exercised by tests on non-Windows builds.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use tauri::Emitter;

/// Logs and emits `close-request {source}`. Rust owns the open/close decision;
/// the frontend just obeys.
#[cfg(target_os = "windows")]
pub fn emit_close_request(app: &tauri::AppHandle, source: &str) {
  log::info!("close-request: source={source}");
  if let Err(err) = app.emit("close-request", serde_json::json!({ "source": source })) {
    log::warn!("failed to emit close-request: {err}");
  }
}


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
  /// Clears any partial tap so a missed Alt-up cannot leave the tracker stuck.
  pub fn reset(&mut self) {
    self.alt_down = false;
    self.clean = true;
    self.pressed_at = None;
    self.last_tap_at = None;
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

/// Fires once per continuous dwell inside the zone.
pub struct DwellTracker {
  since: Option<Instant>,
  fired: bool,
  dwell: Duration,
}

impl DwellTracker {
  pub fn new(ms: u64) -> Self {
    Self {
      since: None,
      fired: false,
      dwell: Duration::from_millis(ms),
    }
  }
  /// Call every poll. Returns true ONCE when the cursor has stayed in the zone long enough.
  pub fn update(&mut self, inside: bool, suppressed: bool, now: Instant) -> bool {
    if !inside || suppressed {
      self.since = None;
      self.fired = false;
      return false;
    }
    let since = *self.since.get_or_insert(now);
    if !self.fired && now.duration_since(since) >= self.dwell {
      self.fired = true;
      return true;
    }
    false
  }
}

/// All values are PHYSICAL pixels. scale = the monitor's scale factor (1.25 at 125%).
pub fn in_hot_zone(cx: i32, cy: i32, mon_x: i32, mon_y: i32, mon_w: i32, scale: f64) -> bool {
  let zone_w = (240.0 * scale) as i32;
  let zone_h = (4.0 * scale).ceil() as i32;
  let left = mon_x + (mon_w - zone_w) / 2;
  cx >= left && cx < left + zone_w && cy >= mon_y && cy < mon_y + zone_h
}

/// True when the hot zone must not fire, regardless of dwell.
pub fn suppressed(mouse_down: bool, foreground_fullscreen: bool, hide_in_fullscreen: bool) -> bool {
  mouse_down || (hide_in_fullscreen && foreground_fullscreen)
}

/// Exposed to the frontend so the compact bar can hide under fullscreen apps.
#[cfg(target_os = "windows")]
pub fn foreground_fullscreen_active() -> bool {
  imp::foreground_fullscreen()
}

#[cfg(not(target_os = "windows"))]
pub fn foreground_fullscreen_active() -> bool {
  false
}

#[cfg(target_os = "windows")]
mod imp {
  use super::{in_hot_zone, suppressed, DwellTracker, DWELL_MS};
  use std::time::{Duration, Instant};
  use tauri::Emitter;
  use windows::Win32::Foundation::{HWND, POINT, RECT};
  use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
  };
  use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
  use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
  };
  use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowRect,
  };

  const POLL_INTERVAL: Duration = Duration::from_millis(16); // ~60 Hz
  const REPORT_INTERVAL: Duration = Duration::from_secs(1);
  /// Desktop/shell windows are never treated as fullscreen apps.
  const SHELL_CLASSES: [&str; 3] = ["Progman", "WorkerW", "Shell_TrayWnd"];

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

  fn monitor_scale(hmonitor: HMONITOR) -> f64 {
    let mut dpi_x = 96u32;
    let mut dpi_y = 96u32;
    if unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.is_ok() {
      dpi_x as f64 / 96.0
    } else {
      1.0
    }
  }

  /// Cursor position (physical px) plus the geometry of the monitor it is on.
  fn cursor_monitor() -> Option<(POINT, RECT, f64)> {
    unsafe {
      let mut point = POINT::default();
      if GetCursorPos(&mut point).is_err() {
        return None;
      }
      let hmonitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
      let rect = monitor_rect(hmonitor)?;
      Some((point, rect, monitor_scale(hmonitor)))
    }
  }

  fn mouse_button_down() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON]
      .iter()
      .any(|vk| (unsafe { GetAsyncKeyState(vk.0 as i32) } as u16) & 0x8000 != 0)
  }

  fn window_class(hwnd: HWND) -> String {
    let mut buffer = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if len <= 0 {
      return String::new();
    }
    String::from_utf16_lossy(&buffer[..len as usize])
  }

  /// True only when the foreground window covers its whole monitor (not the
  /// work area) and is not the desktop/shell. A maximized window matches the
  /// work area, so it does not count.
  pub(super) fn foreground_fullscreen() -> bool {
    unsafe {
      let hwnd: HWND = GetForegroundWindow();
      if hwnd.0.is_null() {
        return false;
      }
      if SHELL_CLASSES.contains(&window_class(hwnd).as_str()) {
        return false;
      }
      let mut window = RECT::default();
      if GetWindowRect(hwnd, &mut window).is_err() {
        return false;
      }
      let Some(monitor) = monitor_rect(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)) else {
        return false;
      };
      window.left == monitor.left
        && window.top == monitor.top
        && window.right == monitor.right
        && window.bottom == monitor.bottom
    }
  }

  pub fn start(app: tauri::AppHandle) {
    let spawned = std::thread::Builder::new()
      .name("float-hot-zone".into())
      .spawn(move || {
        let mut tracker = DwellTracker::new(DWELL_MS);
        let mut last_report: Option<Instant> = None;
        let mut last_reason: Option<(&'static str, Instant)> = None;
        loop {
          let now = Instant::now();
          // Read the live settings every cycle so toggling needs no restart.
          let enabled = crate::settings::top_edge();
          let hide_in_fullscreen = crate::settings::hide_in_fullscreen();

          let hit = cursor_monitor();
          let geometry_inside = hit
            .map(|(point, rect, scale)| {
              in_hot_zone(
                point.x,
                point.y,
                rect.left,
                rect.top,
                rect.right - rect.left,
                scale,
              )
            })
            .unwrap_or(false);
          let inside = enabled && geometry_inside;
          let mouse_down = inside && mouse_button_down();
          let fullscreen = inside && hide_in_fullscreen && foreground_fullscreen();
          let blocked = suppressed(mouse_down, fullscreen, hide_in_fullscreen);

          // Suppression reasons, at most one line per second (and immediately
          // when the reason changes). Only while the cursor is in the zone.
          let reason = if geometry_inside && !enabled {
            Some("top edge disabled in settings")
          } else if mouse_down {
            Some("mouse button held")
          } else if fullscreen {
            Some("fullscreen app detected")
          } else {
            None
          };
          if let Some(reason) = reason {
            let due = last_reason.map_or(true, |(previous, at)| {
              previous != reason || now.duration_since(at) >= REPORT_INTERVAL
            });
            if due {
              log::info!("hover suppressed: {reason}");
              last_reason = Some((reason, now));
            }
          } else {
            last_reason = None;
          }

          // `update` always runs so the state advances even while the panel is
          // open; the event only goes out when the panel is closed.
          if tracker.update(inside, blocked, now) && crate::window_mode::is_closed() {
            log::info!("hot zone dwell reached, emitting open-request (hover)");
            if let Err(err) =
              app.emit(
                "open-request",
                serde_json::json!({ "source": "hover", "pinned": false }),
              )
            {
              log::warn!("failed to emit open-request: {err}");
            }
          }

          if last_report.map_or(true, |at| now.duration_since(at) >= REPORT_INTERVAL) {
            last_report = Some(now);
            match hit {
              Some((point, rect, scale)) => {
                let zone_w = (240.0 * scale) as i32;
                let zone_h = (4.0 * scale).ceil() as i32;
                let left = rect.left + (rect.right - rect.left - zone_w) / 2;
                log::debug!(
                  "hot zone: cursor=({}, {}) zone=({}, {})..({}, {}) scale={scale} inside={inside} suppressed={blocked}",
                  point.x,
                  point.y,
                  left,
                  rect.top,
                  left + zone_w,
                  rect.top + zone_h
                );
              }
              None => log::debug!("hot zone: cursor position unavailable"),
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
  use super::{emit_close_request, AltTapTracker, KeyEvent, ALT_TAP_WINDOW_MS};
  use crate::window_mode::AppState;
  use std::sync::{Mutex, MutexGuard};
  use std::time::Instant;
  use tauri::{Emitter, Manager};
  use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
  use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
  };

  const VK_MENU: u32 = 0x12;
  const VK_LMENU: u32 = 0xA4;
  const VK_RMENU: u32 = 0xA5;
  const VK_ESCAPE: u32 = 0x1B;

  struct HookState {
    tracker: AltTapTracker,
    app: tauri::AppHandle,
  }

  /// A mutex (not thread-local) so `set_ui_mode` can reset the tracker when
  /// the panel opens or closes. Keyboard events are low frequency.
  static STATE: Mutex<Option<HookState>> = Mutex::new(None);

  fn lock_state() -> MutexGuard<'static, Option<HookState>> {
    match STATE.lock() {
      Ok(guard) => guard,
      Err(poisoned) => poisoned.into_inner(),
    }
  }

  fn current_app() -> Option<tauri::AppHandle> {
    lock_state().as_ref().map(|state| state.app.clone())
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
          if alt {
            log::info!(
              "alt {} (vk 0x{:X})",
              if down { "down" } else { "up" },
              info.vkCode
            );
          }

          // Escape closes an open panel even when the webview has no focus.
          if down && info.vkCode == VK_ESCAPE {
            if let Some(app) = current_app() {
              if app.state::<AppState>().is_open() {
                emit_close_request(&app, "esc");
              } else {
                log::debug!("escape ignored (panel closed)");
              }
            }
          }

          let event = match (alt, down) {
            (true, true) => Some(KeyEvent::AltDown),
            (true, false) => Some(KeyEvent::AltUp),
            (false, true) => Some(KeyEvent::OtherDown),
            (false, false) => None,
          };
          if let Some(event) = event {
            let fire = {
              let mut guard = lock_state();
              guard.as_mut().and_then(|state| {
                state
                  .tracker
                  .on_event(event, Instant::now())
                  .then(|| state.app.clone())
              })
            };
            if let Some(app) = fire {
              // Live setting: no restart needed when the toggle changes.
              if !crate::settings::alt_alt() {
                log::debug!("double alt ignored (altAlt off)");
              } else {
                // Rust decides: open when closed, close when open.
                let open = app.state::<AppState>().is_open();
                log::info!(
                  "double alt: panel_open={open}, emitting {}",
                  if open {
                    "close-request (alt)"
                  } else {
                    "open-request (pinned)"
                  }
                );
                if open {
                  emit_close_request(&app, "alt");
                } else if let Err(err) = app.emit(
                  "open-request",
                  serde_json::json!({ "source": "alt", "pinned": true }),
                ) {
                  log::warn!("failed to emit open-request: {err}");
                }
              }
            }
          }
        }
      }
    }
    // SAFETY: plain pass-through; the hook never swallows a key.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
  }

  /// Re-arms the double-Alt sequence. Called by `set_ui_mode` whenever the
  /// panel opens or closes.
  pub fn reset_tracker(reason: &str) {
    let mut guard = lock_state();
    if let Some(state) = guard.as_mut() {
      state.tracker.reset();
      log::info!("alt tracker reset ({reason})");
    }
  }

  /// Installs the low-level keyboard hook on its own thread. The thread must
  /// run a message loop or the hook is never called. `altAlt` is checked live
  /// on every event, so the hook is always installed.
  pub fn start(app: tauri::AppHandle) {
    let spawned = std::thread::Builder::new()
      .name("float-alt-hook".into())
      .spawn(move || {
        *lock_state() = Some(HookState {
          tracker: AltTapTracker::new(ALT_TAP_WINDOW_MS),
          app,
        });

        // SAFETY: hook state is in place; hMod is NULL for a low-level hook,
        // which also ties the hook to this thread.
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
        *lock_state() = None;
        log::info!("keyboard hook removed");
      });

    if let Err(err) = spawned {
      log::error!("failed to start alt hook thread: {err}");
    }
  }
}

#[cfg(target_os = "windows")]
pub use alt_hook::{reset_tracker as reset_alt_tracker, start as start_alt_hook};

#[cfg(not(target_os = "windows"))]
pub fn reset_alt_tracker(_reason: &str) {}

/// Low-level mouse hook: closes a pinned open panel on an outside click.
#[cfg(target_os = "windows")]
mod mouse_hook {
  use super::emit_close_request;
  use crate::window_mode::AppState;
  use std::cell::RefCell;
  use tauri::Manager;
  use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
  use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, MSLLHOOKSTRUCT, MSG, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_MOUSE_LL, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN,
  };

  thread_local! {
    /// The hook is called on the thread that installed it.
    static APP: RefCell<Option<tauri::AppHandle>> = const { RefCell::new(None) };
  }

  /// Pinned + open + click outside the stored panel rect. Hover-opened panels
  /// are unpinned and keep their 400 ms leave-close instead.
  fn should_close(app: &tauri::AppHandle, x: i32, y: i32) -> bool {
    let state = app.state::<AppState>();
    state.is_open() && state.is_pinned() && !state.panel_rect().contains(x, y)
  }

  /// `WH_MOUSE_LL` callback. The click is never swallowed.
  unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
      let message = wparam.0 as u32;
      if matches!(message, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
        // SAFETY: for `WH_MOUSE_LL` with `code >= 0` the system passes an
        // MSLLHOOKSTRUCT pointer.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let point = info.pt;
        if let Some(app) = APP.with(|cell| cell.borrow().clone()) {
          if should_close(&app, point.x, point.y) {
            emit_close_request(&app, "outside");
          }
        }
      }
    }
    // SAFETY: plain pass-through; the hook never swallows a click.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
  }

  /// Installs the mouse hook on its own thread with a message loop.
  pub fn start(app: tauri::AppHandle) {
    let spawned = std::thread::Builder::new()
      .name("float-mouse-hook".into())
      .spawn(move || {
        APP.with(|cell| *cell.borrow_mut() = Some(app));

        // SAFETY: hMod is NULL for a low-level hook, which ties it to this
        // thread and its message loop.
        let hook = match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0) } {
          Ok(hook) => hook,
          Err(err) => {
            log::error!("failed to install mouse hook: {err}");
            return;
          }
        };
        log::info!("mouse hook installed (outside-click close)");

        let mut message = MSG::default();
        loop {
          // SAFETY: blocking wait; the low-level hook runs while we wait here.
          let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
          if result.0 == 0 {
            break;
          }
          if result.0 == -1 {
            log::error!("mouse hook message loop failed");
            break;
          }
        }

        // SAFETY: `hook` came from SetWindowsHookExW on this thread.
        let _ = unsafe { UnhookWindowsHookEx(hook) };
        APP.with(|cell| *cell.borrow_mut() = None);
        log::info!("mouse hook removed");
      });

    if let Err(err) = spawned {
      log::error!("failed to start mouse hook thread: {err}");
    }
  }
}

#[cfg(target_os = "windows")]
pub use mouse_hook::start as start_mouse_hook;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn zone_is_centered_at_scale_1() {
    // 1920 wide: (1920 - 240) / 2 = 840, 4 px tall, half-open right/bottom.
    assert!(in_hot_zone(840, 0, 0, 0, 1920, 1.0));
    assert!(in_hot_zone(1079, 3, 0, 0, 1920, 1.0));
    assert!(!in_hot_zone(839, 0, 0, 0, 1920, 1.0));
    assert!(!in_hot_zone(1080, 0, 0, 0, 1920, 1.0));
    assert!(!in_hot_zone(960, 4, 0, 0, 1920, 1.0));
  }

  #[test]
  fn zone_scales_at_125_and_150_on_1920() {
    // 125%: 300 x 5 starting at 810. 150%: 360 x 6 starting at 780.
    assert!(in_hot_zone(810, 4, 0, 0, 1920, 1.25));
    assert!(in_hot_zone(1109, 0, 0, 0, 1920, 1.25));
    assert!(!in_hot_zone(809, 0, 0, 0, 1920, 1.25));
    assert!(!in_hot_zone(1110, 0, 0, 0, 1920, 1.25));
    assert!(!in_hot_zone(900, 5, 0, 0, 1920, 1.25));

    assert!(in_hot_zone(780, 5, 0, 0, 1920, 1.5));
    assert!(in_hot_zone(1139, 0, 0, 0, 1920, 1.5));
    assert!(!in_hot_zone(779, 0, 0, 0, 1920, 1.5));
    assert!(!in_hot_zone(1140, 0, 0, 0, 1920, 1.5));
    assert!(!in_hot_zone(900, 6, 0, 0, 1920, 1.5));
  }

  #[test]
  fn zone_on_2560_wide_monitor_at_every_scale() {
    assert!(in_hot_zone(1160, 0, 0, 0, 2560, 1.0)); // 240 wide -> 1160..1400
    assert!(in_hot_zone(1399, 3, 0, 0, 2560, 1.0));
    assert!(!in_hot_zone(1400, 0, 0, 0, 2560, 1.0));
    assert!(in_hot_zone(1130, 0, 0, 0, 2560, 1.25)); // 300 wide -> 1130..1430
    assert!(in_hot_zone(1429, 4, 0, 0, 2560, 1.25));
    assert!(!in_hot_zone(1430, 0, 0, 0, 2560, 1.25));
    assert!(in_hot_zone(1100, 0, 0, 0, 2560, 1.5)); // 360 wide -> 1100..1460
    assert!(in_hot_zone(1459, 5, 0, 0, 2560, 1.5));
    assert!(!in_hot_zone(1460, 0, 0, 0, 2560, 1.5));
  }

  #[test]
  fn zone_follows_second_monitor_offset() {
    // Second 1920-wide monitor starting at x = 1920 (scale 1.0).
    assert!(!in_hot_zone(1919, 0, 1920, 0, 1920, 1.0));
    assert!(in_hot_zone(2760, 0, 1920, 0, 1920, 1.0)); // 1920 + 840
    assert!(!in_hot_zone(2759, 0, 1920, 0, 1920, 1.0));
    assert!(!in_hot_zone(3000, 0, 1920, 0, 1920, 1.0));
    // Monitor left of the primary at 125%: 1920 + (1920 - 300) / 2 above 0.
    assert!(in_hot_zone(-1110, 0, -1920, 0, 1920, 1.25));
    assert!(!in_hot_zone(-1111, 0, -1920, 0, 1920, 1.25));
    assert!(!in_hot_zone(-810, 0, -1920, 0, 1920, 1.25));
  }

  #[test]
  fn zone_is_anchored_to_monitor_top_edge() {
    // Monitor above the primary (y = -1080) at 150%.
    assert!(in_hot_zone(840, -1080, 0, -1080, 1920, 1.5));
    assert!(!in_hot_zone(840, -1081, 0, -1080, 1920, 1.5));
    assert!(!in_hot_zone(840, -1074, 0, -1080, 1920, 1.5));
  }

  #[test]
  fn dwell_too_short_does_not_fire() {
    let t0 = Instant::now();
    let mut tracker = DwellTracker::new(DWELL_MS);
    assert!(!tracker.update(true, false, t0));
    assert!(!tracker.update(true, false, t0 + Duration::from_millis(179)));
  }

  #[test]
  fn dwell_fires_at_exactly_180_ms() {
    let t0 = Instant::now();
    let mut tracker = DwellTracker::new(DWELL_MS);
    assert!(!tracker.update(true, false, t0));
    assert!(tracker.update(true, false, t0 + Duration::from_millis(180)));
  }

  #[test]
  fn dwell_fires_once_per_entry() {
    let t0 = Instant::now();
    let mut tracker = DwellTracker::new(DWELL_MS);
    assert!(!tracker.update(true, false, t0));
    assert!(tracker.update(true, false, t0 + Duration::from_millis(200)));
    assert!(!tracker.update(true, false, t0 + Duration::from_millis(400)));
    assert!(!tracker.update(true, false, t0 + Duration::from_millis(600)));
    // Leaving and coming back re-arms exactly one more fire.
    assert!(!tracker.update(false, false, t0 + Duration::from_millis(700)));
    assert!(!tracker.update(true, false, t0 + Duration::from_millis(800)));
    assert!(!tracker.update(true, false, t0 + Duration::from_millis(979)));
    assert!(tracker.update(true, false, t0 + Duration::from_millis(980)));
    assert!(!tracker.update(true, false, t0 + Duration::from_millis(1200)));
  }

  #[test]
  fn suppression_and_leaving_restart_the_dwell() {
    let t0 = Instant::now();
    let mut tracker = DwellTracker::new(DWELL_MS);
    assert!(!tracker.update(true, false, t0));
    // A mouse press (or fullscreen) restarts the clock.
    assert!(!tracker.update(true, true, t0 + Duration::from_millis(100)));
    assert!(!tracker.update(true, false, t0 + Duration::from_millis(200)));
    assert!(!tracker.update(true, false, t0 + Duration::from_millis(379)));
    assert!(tracker.update(true, false, t0 + Duration::from_millis(380)));
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

  #[test]
  fn reset_clears_a_partial_double_tap() {
    let base = Instant::now();
    let mut tracker = AltTapTracker::new(WINDOW);
    assert!(!tap(&mut tracker, base, 0));
    tracker.reset();
    // The tap from before the reset no longer counts.
    assert!(!tap(&mut tracker, base, 100));
    assert!(tap(&mut tracker, base, 200));
  }

  #[test]
  fn reset_clears_a_held_alt() {
    let base = Instant::now();
    let mut tracker = AltTapTracker::new(WINDOW);
    assert!(!tracker.on_event(KeyEvent::AltDown, at(base, 0)));
    tracker.reset();
    // The Alt-up for the key held across the reset is ignored...
    assert!(!tracker.on_event(KeyEvent::AltUp, at(base, 50)));
    // ...and a fresh double tap still fires.
    assert!(!tap(&mut tracker, base, 100));
    assert!(tap(&mut tracker, base, 200));
  }
}
