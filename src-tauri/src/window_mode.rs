//! Window model (docs/ARCHITECTURE.md): idle / pill / open geometry.
//!
//! The frontend reports the desired mode; Rust owns size, position and
//! click-through. Geometry is pure so DPI behaviour is unit-tested.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

/// Extra height below the panel (shadow bleed).
pub const PANEL_BLEED: f64 = 40.0;
pub const OPEN_WIDTH: f64 = 560.0;
pub const PILL_WIDTH: f64 = 200.0;
pub const PILL_HEIGHT: f64 = 90.0;
/// Collapsed bar size from the design (hidden in idle).
pub const IDLE_WIDTH: f64 = 120.0;
pub const IDLE_HEIGHT: f64 = 4.0;
/// Visible panel width from the design (the window is wider for the bleed).
pub const PANEL_WIDTH_LOGICAL: f64 = 480.0;

/// Last mode the frontend asked for. Idle is the only "closed" state, so a
/// live pill still counts as open for hover-trigger purposes.
static PANEL_CLOSED: AtomicBool = AtomicBool::new(true);

/// True while the panel is idle (hidden), i.e. hover may open it.
pub fn is_closed() -> bool {
  PANEL_CLOSED.load(Ordering::Relaxed)
}

/// Physical-px rectangle of the visible panel itself (no shadow bleed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PanelRect {
  pub x: i32,
  pub y: i32,
  pub width: i32,
  pub height: i32,
}

impl PanelRect {
  /// Half-open like the window geometry: right/bottom edges are outside.
  pub fn contains(&self, x: i32, y: i32) -> bool {
    self.width > 0
      && self.height > 0
      && x >= self.x
      && x < self.x + self.width
      && y >= self.y
      && y < self.y + self.height
  }
}

/// Panel height (logical px, as reported by the frontend) + its screen rect.
#[derive(Debug, Clone, Copy, Default)]
struct PanelInfo {
  height: f64,
  rect: PanelRect,
}

/// Rust-owned panel state. Managed with `app.manage(...)`; the trigger hooks
/// read it to decide open/close without asking the frontend.
#[derive(Default)]
pub struct AppState {
  panel_open: AtomicBool,
  panel_pinned: AtomicBool,
  panel: Mutex<PanelInfo>,
}

impl AppState {
  /// True only in `UiMode::Open` (a live pill is not an open panel).
  pub fn is_open(&self) -> bool {
    self.panel_open.load(Ordering::Relaxed)
  }

  /// True when the open panel was opened pinned (keyboard, tray, click).
  pub fn is_pinned(&self) -> bool {
    self.panel_pinned.load(Ordering::Relaxed)
  }

  #[allow(dead_code)]
  pub fn panel_height(&self) -> f64 {
    self.lock_info().height
  }

  /// Screen rect of the visible panel in physical px.
  pub fn panel_rect(&self) -> PanelRect {
    self.lock_info().rect
  }

  fn lock_info(&self) -> std::sync::MutexGuard<'_, PanelInfo> {
    match self.panel.lock() {
      Ok(guard) => guard,
      Err(poisoned) => poisoned.into_inner(),
    }
  }

  fn set(&self, open: bool, pinned: bool, height: f64, rect: PanelRect) {
    self.panel_open.store(open, Ordering::Relaxed);
    self.panel_pinned.store(pinned, Ordering::Relaxed);
    let mut info = self.lock_info();
    info.height = height;
    info.rect = rect;
  }
}

/// Panel rect centered on the monitor's top edge, in physical px.
pub fn panel_screen_rect(
  panel_height: f64,
  monitor_x: i32,
  monitor_y: i32,
  monitor_width: u32,
  scale_factor: f64,
) -> PanelRect {
  let width = (PANEL_WIDTH_LOGICAL * scale_factor).round() as i32;
  let height = (panel_height.max(0.0) * scale_factor).round() as i32;
  PanelRect {
    x: monitor_x + (monitor_width as i32 - width) / 2,
    y: monitor_y,
    width,
    height,
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
  Idle,
  Pill,
  Open,
}

impl UiMode {
  pub fn parse(value: &str) -> Result<Self, String> {
    match value {
      "idle" => Ok(Self::Idle),
      "pill" => Ok(Self::Pill),
      "open" => Ok(Self::Open),
      other => Err(format!("unknown ui mode: {other}")),
    }
  }

  fn logical_size(self, panel_height: f64) -> (f64, f64) {
    match self {
      Self::Idle => (IDLE_WIDTH, IDLE_HEIGHT),
      Self::Pill => (PILL_WIDTH, PILL_HEIGHT),
      Self::Open => (OPEN_WIDTH, panel_height.max(0.0) + PANEL_BLEED),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRect {
  pub x: i32,
  pub y: i32,
  pub width: u32,
  pub height: u32,
}

/// Physical-px rect centered on the monitor's top edge.
pub fn window_rect(
  mode: UiMode,
  panel_height: f64,
  monitor_x: i32,
  monitor_y: i32,
  monitor_width: u32,
  scale_factor: f64,
) -> WindowRect {
  let (logical_width, logical_height) = mode.logical_size(panel_height);
  let width = (logical_width * scale_factor).round() as u32;
  let height = (logical_height * scale_factor).round() as u32;
  let x = monitor_x + (monitor_width as i32 - width as i32) / 2;
  WindowRect {
    x,
    y: monitor_y,
    width,
    height,
  }
}

/// Applies `mode` to the window, anchored to the monitor under the cursor.
/// Also records open/pinned + the panel rect for the trigger hooks.
pub fn apply(
  window: &WebviewWindow,
  state: &AppState,
  mode: UiMode,
  panel_height: f64,
  pinned: bool,
) -> Result<(), String> {
  let app = window.app_handle();
  let cursor = app.cursor_position().map_err(|err| err.to_string())?;
  let monitor = app
    .monitor_from_point(cursor.x, cursor.y)
    .map_err(|err| err.to_string())?
    .or_else(|| window.current_monitor().ok().flatten())
    .or_else(|| app.primary_monitor().ok().flatten())
    .ok_or_else(|| "no monitor available".to_string())?;

  let rect = window_rect(
    mode,
    panel_height,
    monitor.position().x,
    monitor.position().y,
    monitor.size().width,
    monitor.scale_factor(),
  );
  let panel = panel_screen_rect(
    panel_height,
    monitor.position().x,
    monitor.position().y,
    monitor.size().width,
    monitor.scale_factor(),
  );

  window
    .set_size(PhysicalSize::new(rect.width, rect.height))
    .map_err(|err| err.to_string())?;
  window
    .set_position(PhysicalPosition::new(rect.x, rect.y))
    .map_err(|err| err.to_string())?;

  match mode {
    UiMode::Idle => {
      window
        .set_ignore_cursor_events(true)
        .map_err(|err| err.to_string())?;
      window.hide().map_err(|err| err.to_string())?;
    }
    UiMode::Pill | UiMode::Open => {
      window
        .set_ignore_cursor_events(false)
        .map_err(|err| err.to_string())?;
      window.show().map_err(|err| err.to_string())?;
    }
  }

  PANEL_CLOSED.store(mode == UiMode::Idle, Ordering::Relaxed);
  let open = mode == UiMode::Open;
  let was_open = state.is_open();
  state.set(open, open && pinned, panel_height, panel);
  if was_open != open {
    // Re-arm the double-Alt tracker: a missed Alt-up cannot leave it stuck.
    crate::triggers::reset_alt_tracker(if open { "panel opened" } else { "panel closed" });
  }

  Ok(())
}

#[tauri::command]
pub fn set_ui_mode(
  window: WebviewWindow,
  state: tauri::State<'_, AppState>,
  mode: String,
  panel_height: f64,
  pinned: bool,
) -> Result<(), String> {
  apply(
    &window,
    state.inner(),
    UiMode::parse(&mode)?,
    panel_height,
    pinned,
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn open_is_560_wide_and_adds_40() {
    let rect = window_rect(UiMode::Open, 172.0, 0, 0, 1920, 1.0);
    assert_eq!(rect.width, 560);
    assert_eq!(rect.height, 212);
    assert_eq!(rect.x, 680);
    assert_eq!(rect.y, 0);
  }

  #[test]
  fn open_scales_at_125_percent() {
    let rect = window_rect(UiMode::Open, 172.0, 0, 0, 1920, 1.25);
    assert_eq!(rect.width, 700);
    assert_eq!(rect.height, 265);
    assert_eq!(rect.x, 610);
  }

  #[test]
  fn open_scales_at_150_percent() {
    let rect = window_rect(UiMode::Open, 172.0, 0, 0, 1920, 1.5);
    assert_eq!(rect.width, 840);
    assert_eq!(rect.height, 318);
    assert_eq!(rect.x, 540);
  }

  #[test]
  fn pill_is_200_by_90_logical() {
    let rect = window_rect(UiMode::Pill, 340.0, 0, 0, 1920, 1.0);
    assert_eq!((rect.width, rect.height), (200, 90));
    let scaled = window_rect(UiMode::Pill, 340.0, 0, 0, 1920, 1.25);
    assert_eq!((scaled.width, scaled.height), (250, 113));
  }

  #[test]
  fn idle_matches_collapsed_bar() {
    let rect = window_rect(UiMode::Idle, 172.0, 0, 0, 1920, 1.0);
    assert_eq!((rect.width, rect.height), (120, 4));
  }

  #[test]
  fn centers_on_offset_monitor() {
    let rect = window_rect(UiMode::Open, 172.0, 1920, -180, 2560, 1.0);
    assert_eq!(rect.x, 1920 + (2560 - 560) / 2);
    assert_eq!(rect.y, -180);
  }

  #[test]
  fn rejects_unknown_mode() {
    assert!(UiMode::parse("open").is_ok());
    assert!(UiMode::parse("closed").is_err());
  }

  #[test]
  fn panel_rect_is_480_logical_and_centered() {
    let rect = panel_screen_rect(172.0, 0, 0, 1920, 1.0);
    assert_eq!(rect.width, 480);
    assert_eq!(rect.height, 172);
    assert_eq!(rect.x, (1920 - 480) / 2);
    assert_eq!(rect.y, 0);
    assert!(rect.contains(720, 0));
    assert!(rect.contains(1199, 171));
    assert!(!rect.contains(719, 0));
    assert!(!rect.contains(1200, 0));
    assert!(!rect.contains(720, 172));
  }

  #[test]
  fn panel_rect_scales_and_follows_the_monitor() {
    let rect = panel_screen_rect(340.0, 1920, -180, 2560, 1.25);
    assert_eq!(rect.width, 600);
    assert_eq!(rect.height, 425);
    assert_eq!(rect.x, 1920 + (2560 - 600) / 2);
    assert_eq!(rect.y, -180);

    let hidpi = panel_screen_rect(172.0, 0, 0, 1920, 1.5);
    assert_eq!((hidpi.width, hidpi.height), (720, 258));
    assert_eq!(hidpi.x, (1920 - 720) / 2);
  }

  #[test]
  fn app_state_defaults_to_closed_and_unpinned() {
    let state = AppState::default();
    assert!(!state.is_open());
    assert!(!state.is_pinned());
    assert_eq!(state.panel_rect(), PanelRect::default());
    assert!(!state.panel_rect().contains(0, 0));
  }
}
