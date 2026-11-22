//! Window model (docs/ARCHITECTURE.md): idle / pill / open geometry plus the
//! expanding / collapsing / stable handshake used by the shared container.
//!
//! The frontend reports the desired mode; Rust owns size, position and
//! click-through. Geometry is pure so DPI behaviour is unit-tested.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

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

/// Current physical mode/phase, mirrored for trigger diagnostics.
#[derive(Default)]
struct ModePhase {
  mode: UiMode,
  phase: UiPhase,
}

/// A queued trigger plus when it fired. Startup hover/double-Alt must not be
/// replayed long after the user's gesture, so entries expire at `ui_ready`.
struct QueuedTrigger {
  payload: serde_json::Value,
  queued_at: Instant,
}

/// At most one pending payload per trigger kind, delivered after `ui_ready`.
#[derive(Default)]
struct TriggerQueue {
  open: Option<QueuedTrigger>,
  open_notes: Option<QueuedTrigger>,
  close: Option<QueuedTrigger>,
}

/// Longest a queued startup trigger stays relevant (ms).
pub const TRIGGER_MAX_AGE_MS: u64 = 1500;

/// Age of a queued trigger; `None` if the clock moved backwards.
fn trigger_age_ms(queued_at: Instant, now: Instant) -> Option<u64> {
  now.checked_duration_since(queued_at)
    .map(|age| age.as_millis() as u64)
}

/// Deliver only triggers younger than `TRIGGER_MAX_AGE_MS`.
fn is_fresh_trigger(queued_at: Instant, now: Instant) -> bool {
  matches!(
    trigger_age_ms(queued_at, now),
    Some(age) if age < TRIGGER_MAX_AGE_MS
  )
}

/// Rust-owned panel state. Managed with `app.manage(...)`; the trigger hooks
/// read it to decide open/close without asking the frontend.
#[derive(Default)]
pub struct AppState {
  panel_open: AtomicBool,
  panel_pinned: AtomicBool,
  panel: Mutex<PanelInfo>,
  mode_phase: Mutex<ModePhase>,
  ui_ready: AtomicBool,
  queued: Mutex<TriggerQueue>,
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

  /// False until the webview has registered every Tauri listener.
  pub fn is_ui_ready(&self) -> bool {
    self.ui_ready.load(Ordering::Relaxed)
  }

  /// Phase/mode the trigger diagnostics report.
  pub fn mode(&self) -> UiMode {
    self.lock_mode_phase().mode
  }

  pub fn phase(&self) -> UiPhase {
    self.lock_mode_phase().phase
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

  fn lock_mode_phase(&self) -> std::sync::MutexGuard<'_, ModePhase> {
    match self.mode_phase.lock() {
      Ok(guard) => guard,
      Err(poisoned) => poisoned.into_inner(),
    }
  }

  fn lock_queue(&self) -> std::sync::MutexGuard<'_, TriggerQueue> {
    match self.queued.lock() {
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

  fn set_mode_phase(&self, mode: UiMode, phase: UiPhase) {
    let mut info = self.lock_mode_phase();
    info.mode = mode;
    info.phase = phase;
  }

  /// Forces the closed/idle bookkeeping even when `apply` dedupes geometry.
  fn mark_closed(&self) {
    PANEL_CLOSED.store(true, Ordering::Relaxed);
    self.panel_open.store(false, Ordering::Relaxed);
    self.panel_pinned.store(false, Ordering::Relaxed);
  }

  fn set_ui_ready(&self) {
    self.ui_ready.store(true, Ordering::Relaxed);
  }

  fn queue(&self, kind: TriggerKind, payload: serde_json::Value) {
    let entry = QueuedTrigger {
      payload,
      queued_at: Instant::now(),
    };
    let mut queue = self.lock_queue();
    match kind {
      TriggerKind::Open => queue.open = Some(entry),
      TriggerKind::OpenNotes => queue.open_notes = Some(entry),
      TriggerKind::Close => queue.close = Some(entry),
    }
  }

  /// Drains queued triggers, open before close. Used once by `ui_ready`.
  fn take_queued(&self) -> Vec<(TriggerKind, QueuedTrigger)> {
    let mut queue = self.lock_queue();
    let mut out = Vec::new();
    if let Some(entry) = queue.open.take() {
      out.push((TriggerKind::Open, entry));
    }
    if let Some(entry) = queue.open_notes.take() {
      out.push((TriggerKind::OpenNotes, entry));
    }
    if let Some(entry) = queue.close.take() {
      out.push((TriggerKind::Close, entry));
    }
    out
  }
}

/// Trigger event kinds; at most one of each is queued before `ui_ready`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
  Open,
  OpenNotes,
  Close,
}

impl TriggerKind {
  fn event(self) -> &'static str {
    match self {
      Self::Open => "open-request",
      Self::OpenNotes => "open-notes",
      Self::Close => "close-request",
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
  #[default]
  Idle,
  Pill,
  Open,
}

/// Frontend/Rust handshake for a morph. Expanding grows before content moves;
/// collapsing holds the source geometry until Motion has settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiPhase {
  #[default]
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

/// What `apply` actually did, for the `FLOAT_DEBUG=1` plan log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyOutcome {
  pub deduped: bool,
  pub visible: bool,
  pub width: u32,
  pub height: u32,
  pub ignore_cursor: bool,
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
  // Logical panel state from the target surface. During a collapse the
  // physical `mode` is still Open while this is already false, so triggers
  // (double-Alt, Esc, outside click) act on the intended state immediately.
  open: bool,
) -> Result<ApplyOutcome, String> {
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
  state.set_mode_phase(mode, phase);
  let unchanged = {
    let mut applied = lock_applied();
    if *applied == Some(desired) {
      true
    } else {
      *applied = Some(desired);
      false
    }
  };
  let outcome = ApplyOutcome {
    deduped: unchanged,
    visible,
    width: rect.width,
    height: rect.height,
    ignore_cursor,
  };
  if unchanged {
    return Ok(outcome);
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

  PANEL_CLOSED.store(!open, Ordering::Relaxed);
  let was_open = state.is_open();
  state.set(open, pinned_state, panel_height, panel);
  if was_open != open {
    // Re-arm the double-Alt tracker: a missed Alt-up cannot leave it stuck.
    crate::triggers::reset_alt_tracker(if open { "panel opened" } else { "panel closed" });
  }

  Ok(outcome)
}

/// Increments on every `set_ui_mode` the frontend sends. The open-request
/// watchdog compares this before/after its 400 ms wait.
static PLAN_SEQ: AtomicU64 = AtomicU64::new(0);

/// How long Rust waits for a plan after an open-request before forcing one.
const OPEN_WATCHDOG_MS: u64 = 400;
/// Panel height used by the watchdog when no plan has ever supplied one.
const WATCHDOG_PANEL_HEIGHT: f64 = 172.0;

fn note_plan_received() {
  PLAN_SEQ.fetch_add(1, Ordering::Relaxed);
}

/// Emits a trigger once the webview has registered its listeners. Before
/// `ui_ready`, only the newest payload per kind is kept: startup hover or
/// double-Alt is then delivered instead of being emitted into a page with no
/// listener yet.
pub fn emit_trigger(app: &tauri::AppHandle, kind: TriggerKind, payload: serde_json::Value) {
  let state = app.state::<AppState>();
  let debug = crate::music::debug_enabled();
  if !state.is_ui_ready() {
    state.queue(kind, payload.clone());
    log::info!(
      "[trigger] {} queued until ui_ready logical_open={} mode={:?} phase={:?} payload={}",
      kind.event(),
      state.is_open(),
      state.mode(),
      state.phase(),
      payload
    );
    return;
  }
  if debug {
    log::info!(
      "[trigger] {} emitted logical_open={} mode={:?} phase={:?} payload={}",
      kind.event(),
      state.is_open(),
      state.mode(),
      state.phase(),
      payload
    );
  } else {
    log::info!("[trigger] {} emitted payload={}", kind.event(), payload);
  }
  if let Err(err) = app.emit(kind.event(), payload.clone()) {
    log::warn!("failed to emit {}: {err}", kind.event());
  }
  if kind == TriggerKind::Open {
    start_open_watchdog(app, payload);
  }
}

pub fn emit_open_request(app: &tauri::AppHandle, payload: serde_json::Value) {
  emit_trigger(app, TriggerKind::Open, payload);
}

pub fn emit_open_notes_request(app: &tauri::AppHandle, payload: serde_json::Value) {
  emit_trigger(app, TriggerKind::OpenNotes, payload);
}

pub fn emit_close_request(app: &tauri::AppHandle, payload: serde_json::Value) {
  emit_trigger(app, TriggerKind::Close, payload);
}

/// If the frontend does not answer an open-request with a plan within 400 ms,
/// force the open geometry (visible, interactive) and re-emit once. This is a
/// safety net for a suspended webview; it never arms a second watchdog.
fn start_open_watchdog(app: &tauri::AppHandle, payload: serde_json::Value) {
  let token = PLAN_SEQ.load(Ordering::Relaxed);
  let app = app.clone();
  let spawned = std::thread::Builder::new()
    .name("float-open-watchdog".into())
    .spawn(move || {
      std::thread::sleep(Duration::from_millis(OPEN_WATCHDOG_MS));
      if PLAN_SEQ.load(Ordering::Relaxed) != token {
        return;
      }
      log::warn!(
        "no set_ui_mode within {OPEN_WATCHDOG_MS} ms after open-request; forcing open geometry and re-emitting once"
      );
      if let Some(window) = app.get_webview_window("main") {
        let state = app.state::<AppState>();
        let height = state.panel_height();
        let height = if height > 0.0 {
          height
        } else {
          WATCHDOG_PANEL_HEIGHT
        };
        // Keep keyboard/tray opens pinned so an outside click still closes them.
        let pinned = payload
          .get("pinned")
          .and_then(|value| value.as_bool())
          .unwrap_or(false);
        match apply(
          &window,
          state.inner(),
          UiMode::Open,
          UiPhase::Stable,
          height,
          0.0,
          0.0,
          pinned,
          true,
        ) {
          Ok(outcome) => log::warn!(
            "open watchdog applied: deduped={} visible={} size={}x{} ignore_cursor={}",
            outcome.deduped,
            outcome.visible,
            outcome.width,
            outcome.height,
            outcome.ignore_cursor
          ),
          Err(err) => log::warn!("open watchdog apply failed: {err}"),
        }
      }
      if let Err(err) = app.emit("open-request", payload) {
        log::warn!("open watchdog re-emit failed: {err}");
      }
    });
  if let Err(err) = spawned {
    log::warn!("failed to start open-request watchdog: {err}");
  }
}

/// Called by the frontend after every Tauri listener is registered: reset Rust
/// to idle/stable/logical_open=false, then deliver anything queued.
#[tauri::command]
pub fn ui_ready(
  window: WebviewWindow,
  state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
  let debug = crate::music::debug_enabled();
  let panel_height = state.panel_height();
  let outcome = apply(
    &window,
    state.inner(),
    UiMode::Idle,
    UiPhase::Stable,
    panel_height,
    0.0,
    0.0,
    false,
    false,
  )?;
  state.mark_closed();
  crate::triggers::reset_alt_tracker("ui ready (idle)");
  // Take before flipping ready, then take again: a trigger racing this call is
  // either queued (and delivered) or emitted directly, never dropped.
  let mut queued = state.take_queued();
  state.set_ui_ready();
  queued.extend(state.take_queued());
  if debug {
    log::info!(
      "[window] ui_ready: idle/stable logical_open=false ({}) visible={} size={}x{} ignore_cursor={}",
      if outcome.deduped { "deduped" } else { "applied" },
      window.is_visible().unwrap_or(false),
      outcome.width,
      outcome.height,
      outcome.ignore_cursor
    );
  }
  if debug {
    log::info!(
      "[window] ui_ready delivering {} queued trigger(s)",
      queued.len()
    );
  }
  for (kind, entry) in queued {
    let now = Instant::now();
    if is_fresh_trigger(entry.queued_at, now) {
      emit_trigger(&window.app_handle(), kind, entry.payload);
    } else {
      match trigger_age_ms(entry.queued_at, now) {
        Some(age) => log::info!("dropped stale trigger (age {age} ms)"),
        None => log::info!("dropped stale trigger (age unknown)"),
      }
    }
  }
  if let Some(tray) = window.app_handle().tray_by_id("main") {
    if let Err(err) = tray.set_tooltip(Some("Float")) {
      log::warn!("failed to update tray tooltip: {err}");
    }
  }
  Ok(debug)
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
  // Optional for older frontends; defaults to the physical mode.
  logical_open: Option<bool>,
) -> Result<(), String> {
  let debug = crate::music::debug_enabled();
  let parsed_mode = UiMode::parse(&mode)?;
  let parsed_phase = UiPhase::parse(phase.as_deref());
  let open = logical_open.unwrap_or(parsed_mode == UiMode::Open);
  if debug {
    log::info!(
      "[window] set_ui_mode received mode={:?} phase={:?} logical_open={open} panel_height={panel_height} bar={bar_width}x{bar_height} pinned={pinned} window_visible={}",
      parsed_mode,
      parsed_phase,
      window.is_visible().unwrap_or(false)
    );
  }
  note_plan_received();
  let outcome = apply(
    &window,
    state.inner(),
    parsed_mode,
    parsed_phase,
    panel_height,
    bar_width,
    bar_height,
    pinned,
    open,
  )?;
  if debug {
    log::info!(
      "[window] set_ui_mode {} mode={:?} phase={:?} logical_open={open} visible={} size={}x{} ignore_cursor={} window_visible={}",
      if outcome.deduped { "deduped" } else { "applied" },
      parsed_mode,
      parsed_phase,
      outcome.visible,
      outcome.width,
      outcome.height,
      outcome.ignore_cursor,
      window.is_visible().unwrap_or(false)
    );
  }
  Ok(())
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
    assert!(!state.is_ui_ready());
    assert_eq!(state.mode(), UiMode::Idle);
    assert_eq!(state.phase(), UiPhase::Stable);
    assert_eq!(state.panel_rect(), PanelRect::default());
    assert!(!state.panel_rect().contains(0, 0));
  }

  #[test]
  fn triggers_queue_one_payload_per_kind_until_ui_ready() {
    let state = AppState::default();
    state.queue(TriggerKind::Open, serde_json::json!({ "source": "hover" }));
    // A second open replaces the first: only the newest startup trigger counts.
    state.queue(TriggerKind::Open, serde_json::json!({ "source": "alt" }));
    state.queue(
      TriggerKind::OpenNotes,
      serde_json::json!({ "source": "hotkey" }),
    );
    state.queue(TriggerKind::Close, serde_json::json!({ "source": "esc" }));

    let queued = state.take_queued();
    assert_eq!(queued.len(), 3);
    assert_eq!(queued[0].0, TriggerKind::Open);
    assert_eq!(queued[0].1.payload["source"], "alt");
    assert_eq!(queued[1].0, TriggerKind::OpenNotes);
    assert_eq!(queued[1].1.payload["source"], "hotkey");
    assert_eq!(queued[2].0, TriggerKind::Close);
    assert_eq!(queued[2].1.payload["source"], "esc");
    // Draining clears the queue so ui_ready delivers each kind once.
    assert!(state.take_queued().is_empty());
  }

  #[test]
  fn ui_ready_is_sticky() {
    let state = AppState::default();
    assert!(!state.is_ui_ready());
    state.set_ui_ready();
    assert!(state.is_ui_ready());
  }

  #[test]
  fn trigger_event_names_match_the_frontend_contract() {
    assert_eq!(TriggerKind::Open.event(), "open-request");
    assert_eq!(TriggerKind::OpenNotes.event(), "open-notes");
    assert_eq!(TriggerKind::Close.event(), "close-request");
  }

  #[test]
  fn queued_triggers_expire_after_1500_ms() {
    let queued_at = Instant::now();
    // Fresh enough: same tick, and just inside the limit.
    assert!(is_fresh_trigger(queued_at, queued_at));
    assert!(is_fresh_trigger(
      queued_at,
      queued_at + Duration::from_millis(TRIGGER_MAX_AGE_MS - 1)
    ));
    // At/over the limit (and clock skew) are dropped at ui_ready.
    assert!(!is_fresh_trigger(
      queued_at,
      queued_at + Duration::from_millis(TRIGGER_MAX_AGE_MS)
    ));
    assert!(!is_fresh_trigger(
      queued_at,
      queued_at + Duration::from_millis(TRIGGER_MAX_AGE_MS * 2)
    ));
    assert!(!is_fresh_trigger(
      queued_at + Duration::from_secs(1),
      queued_at
    ));

    assert_eq!(
      trigger_age_ms(queued_at, queued_at + Duration::from_millis(750)),
      Some(750)
    );
    assert_eq!(trigger_age_ms(queued_at + Duration::from_secs(1), queued_at), None);
  }
}
