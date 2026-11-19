//! Window model (docs/ARCHITECTURE.md): idle / pill / open geometry plus the
//! expanding / collapsing / stable handshake used by the shared container.
//!
//! The frontend reports the desired mode; Rust owns size, position and
//! click-through. Geometry is pure so DPI behaviour is unit-tested.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

/// Extra height below the panel (shadow bleed).
pub const PANEL_BLEED: f64 = 40.0;
pub const OPEN_WIDTH: f64 = 560.0;
/// Total transparent margin around the compact bar (16 px each side).
pub const PILL_MARGIN_X: f64 = 32.0;
/// Transparent margin below the compact bar (room for the glow).
pub const PILL_MARGIN_Y: f64 = 18.0;
/// Used only when an old frontend sends no bar dimensions.
pub const DEFAULT_BAR_WIDTH: f64 = 150.0;
pub const DEFAULT_BAR_HEIGHT: f64 = 30.0;
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

/// Frontend/Rust handshake for a morph. Expanding grows before content moves;
/// collapsing holds the source geometry until Motion has settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiPhase {
  Stable,
  Expanding,
  Collapsing,
}

impl UiPhase {
  /// Missing or unknown phases are stable, so older frontends still work.
  pub fn parse(value: Option<&str>) -> Self {
    match value.unwrap_or("stable") {
      "expanding" => Self::Expanding,
      "collapsing" => Self::Collapsing,
      _ => Self::Stable,
    }
  }
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

  fn logical_size(self, panel_height: f64, bar_width: f64, bar_height: f64) -> (f64, f64) {
    match self {
      Self::Idle => (IDLE_WIDTH, IDLE_HEIGHT),
      Self::Pill => {
        let width = if bar_width > 0.0 {
          bar_width
        } else {
          DEFAULT_BAR_WIDTH
        };
        let height = if bar_height > 0.0 {
          bar_height
        } else {
          DEFAULT_BAR_HEIGHT
        };
        (width + PILL_MARGIN_X, height + PILL_MARGIN_Y)
      }
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

/// Last geometry actually applied to the WebviewWindow. Used to drop repeated
/// `set_ui_mode` calls from ResizeObserver and animation completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AppliedGeometry {
  rect: WindowRect,
  phase: UiPhase,
  ignore_cursor: bool,
  visible: bool,
  open: bool,
  pinned: bool,
}

static APPLIED: Mutex<Option<AppliedGeometry>> = Mutex::new(None);

fn lock_applied() -> std::sync::MutexGuard<'static, Option<AppliedGeometry>> {
  match APPLIED.lock() {
    Ok(guard) => guard,
    Err(poisoned) => poisoned.into_inner(),
  }
}

/// Physical-px rect centered on the monitor's top edge.
pub fn window_rect(
  mode: UiMode,
  panel_height: f64,
  bar_width: f64,
  bar_height: f64,
  monitor_x: i32,
  monitor_y: i32,
  monitor_width: u32,
  scale_factor: f64,
) -> WindowRect {
  let (logical_width, logical_height) = mode.logical_size(panel_height, bar_width, bar_height);
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
  phase: UiPhase,
  panel_height: f64,
  bar_width: f64,
  bar_height: f64,
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
    bar_width,
    bar_height,
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

  // During a morph the source window is held and made click-through, so no
  // large transparent rectangle steals clicks from the desktop behind it.
  let (ignore_cursor, visible) = match phase {
    UiPhase::Stable => match mode {
      UiMode::Idle => (true, false),
      UiMode::Pill | UiMode::Open => (false, true),
    },
    UiPhase::Expanding | UiPhase::Collapsing => (true, mode != UiMode::Idle),
  };
  let open = mode == UiMode::Open;
  let pinned_state = if open {
    if phase == UiPhase::Stable {
      pinned
    } else {
      pinned || state.is_pinned()
    }
  } else {
    false
  };

  let desired = AppliedGeometry {
    rect,
    phase,
    ignore_cursor,
    visible,
    open,
    pinned: pinned_state,
  };
  let unchanged = {
    let mut applied = lock_applied();
    if *applied == Some(desired) {
      true
    } else {
      *applied = Some(desired);
      false
    }
  };
  if unchanged {
    return Ok(());
  }

  window
    .set_size(PhysicalSize::new(rect.width, rect.height))
    .map_err(|err| err.to_string())?;
  window
    .set_position(PhysicalPosition::new(rect.x, rect.y))
    .map_err(|err| err.to_string())?;
  window
    .set_ignore_cursor_events(ignore_cursor)
    .map_err(|err| err.to_string())?;
  if visible {
    window.show().map_err(|err| err.to_string())?;
  } else {
    window.hide().map_err(|err| err.to_string())?;
  }

  PANEL_CLOSED.store(mode == UiMode::Idle, Ordering::Relaxed);
  let was_open = state.is_open();
  state.set(open, pinned_state, panel_height, panel);
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
  bar_width: f64,
  bar_height: f64,
  pinned: bool,
  phase: Option<String>,
) -> Result<(), String> {
  apply(
    &window,
    state.inner(),
    UiMode::parse(&mode)?,
    UiPhase::parse(phase.as_deref()),
    panel_height,
    bar_width,
    bar_height,
    pinned,
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn open_is_560_wide_and_adds_40() {
    let rect = window_rect(UiMode::Open, 172.0, 0.0, 0.0, 0, 0, 1920, 1.0);
    assert_eq!(rect.width, 560);
    assert_eq!(rect.height, 212);
    assert_eq!(rect.x, 680);
    assert_eq!(rect.y, 0);
  }

  #[test]
  fn open_scales_at_125_percent() {
    let rect = window_rect(UiMode::Open, 172.0, 0.0, 0.0, 0, 0, 1920, 1.25);
    assert_eq!(rect.width, 700);
    assert_eq!(rect.height, 265);
    assert_eq!(rect.x, 610);
  }

  #[test]
  fn open_scales_at_150_percent() {
    let rect = window_rect(UiMode::Open, 172.0, 0.0, 0.0, 0, 0, 1920, 1.5);
    assert_eq!(rect.width, 840);
    assert_eq!(rect.height, 318);
    assert_eq!(rect.x, 540);
  }

  #[test]
  fn pill_uses_the_bar_size_plus_margins() {
    let rect = window_rect(UiMode::Pill, 340.0, 150.0, 30.0, 0, 0, 1920, 1.0);
    assert_eq!((rect.width, rect.height), (182, 48));
    assert_eq!(rect.x, (1920 - 182) / 2);

    let wide = window_rect(UiMode::Pill, 340.0, 260.0, 30.0, 0, 0, 1920, 1.0);
    assert_eq!((wide.width, wide.height), (292, 48));

    let scaled = window_rect(UiMode::Pill, 340.0, 150.0, 30.0, 0, 0, 1920, 1.25);
    assert_eq!((scaled.width, scaled.height), (228, 60));
  }

  #[test]
  fn pill_falls_back_when_dimensions_are_missing() {
    let rect = window_rect(UiMode::Pill, 340.0, 0.0, 0.0, 0, 0, 1920, 1.0);
    assert_eq!((rect.width, rect.height), (182, 48));
  }

  #[test]
  fn idle_matches_collapsed_bar() {
    let rect = window_rect(UiMode::Idle, 172.0, 0.0, 0.0, 0, 0, 1920, 1.0);
    assert_eq!((rect.width, rect.height), (120, 4));
  }

  #[test]
  fn centers_on_offset_monitor() {
    let rect = window_rect(UiMode::Open, 172.0, 0.0, 0.0, 1920, -180, 2560, 1.0);
    assert_eq!(rect.x, 1920 + (2560 - 560) / 2);
    assert_eq!(rect.y, -180);
  }

  #[test]
  fn rejects_unknown_mode() {
    assert!(UiMode::parse("open").is_ok());
    assert!(UiMode::parse("closed").is_err());
  }

  #[test]
  fn phases_default_to_stable_for_older_frontends() {
    assert_eq!(UiPhase::parse(None), UiPhase::Stable);
    assert_eq!(UiPhase::parse(Some("stable")), UiPhase::Stable);
    assert_eq!(UiPhase::parse(Some("expanding")), UiPhase::Expanding);
    assert_eq!(UiPhase::parse(Some("collapsing")), UiPhase::Collapsing);
    assert_eq!(UiPhase::parse(Some("unknown")), UiPhase::Stable);
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
