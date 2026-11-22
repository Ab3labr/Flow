//! Rebindable global hotkeys.
//!
//! The parser, status model and duplicate rules are pure so they can be
//! unit-tested everywhere. Registration uses Win32 `RegisterHotKey` with
//! `MOD_NOREPEAT` on the existing keyboard-hook message thread; no media keys
//! are simulated and no external process is contacted.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex, RwLock};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::settings::NotchSettings;

/// Event carrying registration failures to the Settings tab.
pub const HOTKEY_STATUS_EVENT: &str = "hotkeys-status";

const MOD_ALT_BIT: u32 = 0x0001;
const MOD_CONTROL_BIT: u32 = 0x0002;
const MOD_SHIFT_BIT: u32 = 0x0004;
const MOD_WIN_BIT: u32 = 0x0008;
/// `MOD_NOREPEAT`: held keys fire once.
const MOD_NOREPEAT_BIT: u32 = 0x4000;

/// A failed binding is `Some(reason)`; `None` means registered or disabled.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyFailures {
  pub notes: Option<String>,
  pub next: Option<String>,
  pub prev: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
  OpenNotes,
  NextTrack,
  PrevTrack,
}

impl HotkeyAction {
  const ALL: [Self; 3] = [Self::OpenNotes, Self::NextTrack, Self::PrevTrack];

  fn id(self) -> i32 {
    match self {
      Self::OpenNotes => 1,
      Self::NextTrack => 2,
      Self::PrevTrack => 3,
    }
  }

  fn from_id(id: i32) -> Option<Self> {
    Self::ALL.into_iter().find(|action| action.id() == id)
  }

  fn field(self) -> &'static str {
    match self {
      Self::OpenNotes => "notes",
      Self::NextTrack => "next",
      Self::PrevTrack => "prev",
    }
  }

  fn binding(self, settings: &NotchSettings) -> &str {
    match self {
      Self::OpenNotes => &settings.hotkey_notes,
      Self::NextTrack => &settings.hotkey_next,
      Self::PrevTrack => &settings.hotkey_prev,
    }
  }
}

/// Parsed modifier mask + virtual key, with a canonical comparison string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedShortcut {
  modifiers: u32,
  vk: u32,
  canonical: String,
}

impl ParsedShortcut {
  pub fn modifiers(&self) -> u32 {
    self.modifiers
  }

  pub fn key_code(&self) -> u32 {
    self.vk
  }

  pub fn canonical(&self) -> &str {
    &self.canonical
  }
}

fn key_code(token: &str) -> Option<u32> {
  let bytes = token.as_bytes();
  if bytes.len() == 1 {
    if bytes[0].is_ascii_uppercase() {
      return Some(0x41 + (bytes[0] - b'A') as u32);
    }
    if bytes[0].is_ascii_digit() {
      return Some(0x30 + (bytes[0] - b'0') as u32);
    }
  }
  if let Some(number) = token.strip_prefix('F') {
    if let Ok(value) = number.parse::<u32>() {
      if (1..=24).contains(&value) {
        return Some(0x6F + value);
      }
    }
  }
  match token {
    "BACKSPACE" => Some(0x08),
    "TAB" => Some(0x09),
    "ENTER" => Some(0x0D),
    "ESC" | "ESCAPE" => Some(0x1B),
    "SPACE" => Some(0x20),
    "PAGEUP" => Some(0x21),
    "PAGEDOWN" => Some(0x22),
    "END" => Some(0x23),
    "HOME" => Some(0x24),
    "LEFT" => Some(0x25),
    "UP" => Some(0x26),
    "RIGHT" => Some(0x27),
    "DOWN" => Some(0x28),
    "INSERT" => Some(0x2D),
    "DELETE" => Some(0x2E),
    _ => None,
  }
}

/// Parses `Alt+N` and `Ctrl + Alt + F`. At least one modifier and a real key
/// are required, so the double-Alt literal (`Alt Alt`) is not a global binding.
pub fn parse_shortcut(input: &str) -> Option<ParsedShortcut> {
  let mut modifiers = 0u32;
  let mut modifier_names: Vec<String> = Vec::new();
  let mut key: Option<(String, u32)> = None;

  for raw in input.split('+') {
    let token = raw.trim().to_ascii_uppercase();
    if token.is_empty() {
      return None;
    }
    let modifier = match token.as_str() {
      "CTRL" | "CONTROL" => Some((MOD_CONTROL_BIT, "Ctrl")),
      "ALT" => Some((MOD_ALT_BIT, "Alt")),
      "SHIFT" => Some((MOD_SHIFT_BIT, "Shift")),
      "META" | "WIN" | "WINDOWS" | "SUPER" => Some((MOD_WIN_BIT, "Meta")),
      _ => None,
    };
    if let Some((bit, name)) = modifier {
      if modifiers & bit == 0 {
        modifiers |= bit;
        modifier_names.push(name.to_string());
      }
      continue;
    }
    if key.is_some() {
      return None;
    }
    key = Some((token.clone(), key_code(&token)?));
  }

  if modifiers == 0 {
    return None;
  }
  let (key_name, vk) = key?;
  let order = ["Ctrl", "Alt", "Shift", "Meta"];
  modifier_names.sort_by_key(|name| order.iter().position(|item| item == name).unwrap_or(99));
  let mut canonical_parts = modifier_names;
  canonical_parts.push(key_name);
  Some(ParsedShortcut {
    modifiers,
    vk,
    canonical: canonical_parts.join(" + "),
  })
}

static FAILURES: LazyLock<RwLock<HotkeyFailures>> =
  LazyLock::new(|| RwLock::new(HotkeyFailures::default()));
static PENDING: LazyLock<Mutex<Option<NotchSettings>>> = LazyLock::new(|| Mutex::new(None));
static THREAD_ID: AtomicU32 = AtomicU32::new(0);

pub fn status() -> HotkeyFailures {
  match FAILURES.read() {
    Ok(guard) => guard.clone(),
    Err(poisoned) => poisoned.into_inner().clone(),
  }
}

fn publish_status(app: &tauri::AppHandle, next: HotkeyFailures) {
  {
    let mut guard = match FAILURES.write() {
      Ok(guard) => guard,
      Err(poisoned) => poisoned.into_inner(),
    };
    *guard = next.clone();
  }
  if let Err(err) = app.emit(HOTKEY_STATUS_EVENT, next) {
    log::warn!("failed to emit {HOTKEY_STATUS_EVENT}: {err}");
  }
}

/// Defensive duplicate/validity check. The Settings UI enforces the same rules
/// before saving; this keeps the Rust registration path safe regardless.
fn parsed_bindings(settings: &NotchSettings) -> Vec<(HotkeyAction, Option<ParsedShortcut>)> {
  let parsed: Vec<_> = HotkeyAction::ALL
    .into_iter()
    .map(|action| (action, parse_shortcut(action.binding(settings))))
    .collect();

  let double_alt = settings.shortcut.trim().to_ascii_uppercase().replace(" + ", " ");
  let mut seen: Vec<String> = Vec::new();
  parsed
    .into_iter()
    .map(|(action, parsed)| {
      let Some(parsed) = parsed else {
        return (action, None);
      };
      let canonical_key = parsed.canonical.to_ascii_uppercase().replace(" + ", " ");
      if seen.iter().any(|item| item == &parsed.canonical)
        || canonical_key == double_alt
      {
        return (action, None);
      }
      seen.push(parsed.canonical.clone());
      (action, Some(parsed))
    })
    .collect()
}

fn register_all(app: &tauri::AppHandle, settings: &NotchSettings) {
  let mut failures = HotkeyFailures::default();
  let bindings = parsed_bindings(settings);

  for action in HotkeyAction::ALL {
    // Always clear the previous id so a rebound shortcut cannot leave a stale
    // registration behind.
    #[cfg(target_os = "windows")]
    imp::unregister(action);
  }

  if !settings.hotkeys_enabled {
    publish_status(app, failures);
    log::info!("hotkeys: disabled");
    return;
  }

  for (action, parsed) in bindings {
    if parsed.is_none() {
      let reason = if parse_shortcut(action.binding(settings)).is_none() {
        "Invalid shortcut"
      } else {
        "Duplicate shortcut"
      };
      log::error!(
        "hotkey {} ({}) not registered: {reason}",
        action.field(),
        action.binding(settings)
      );
      match action {
        HotkeyAction::OpenNotes => failures.notes = Some(reason.to_string()),
        HotkeyAction::NextTrack => failures.next = Some(reason.to_string()),
        HotkeyAction::PrevTrack => failures.prev = Some(reason.to_string()),
      }
      continue;
    }
    let parsed = parsed.unwrap_or_else(|| unreachable!());
    #[cfg(target_os = "windows")]
    if let Err(err) = imp::register(action, &parsed) {
      log::error!(
        "hotkey {} ({}) already in use or unavailable: {err}",
        action.field(),
        parsed.canonical()
      );
      let message = "In use by another app".to_string();
      match action {
        HotkeyAction::OpenNotes => failures.notes = Some(message),
        HotkeyAction::NextTrack => failures.next = Some(message),
        HotkeyAction::PrevTrack => failures.prev = Some(message),
      }
    } else {
      log::info!("hotkey {} registered: {}", action.field(), parsed.canonical());
    }
    #[cfg(not(target_os = "windows"))]
    log::info!("hotkey {} parsed: {}", action.field(), parsed.canonical());
  }
  publish_status(app, failures);
}

fn media_command(app: &tauri::AppHandle, command: crate::music::MediaCommand, label: &str) {
  let state = app.state::<std::sync::Arc<crate::music::MediaState>>().inner().clone();
  let label = label.to_string();
  tauri::async_runtime::spawn_blocking(move || {
    let ok = state.send_command(command, crate::music::COMMAND_TIMEOUT);
    if !ok {
      log::info!("hotkey {label}: no displayed media session");
    }
  });
}

fn handle_hotkey(app: &tauri::AppHandle, id: i32) {
  match HotkeyAction::from_id(id) {
    Some(HotkeyAction::OpenNotes) => {
      if !crate::settings::hotkeys_enabled() {
        log::info!("hotkey open-notes ignored (hotkeysEnabled off)");
        return;
      }
      if crate::settings::hide_in_fullscreen() && crate::triggers::foreground_fullscreen_active() {
        log::info!("hotkey open-notes ignored (fullscreen app in front)");
        return;
      }
      crate::window_mode::emit_open_notes_request(
        app,
        serde_json::json!({ "source": "hotkey", "pinned": true }),
      );
      #[cfg(target_os = "windows")]
      imp::focus_window(app);
    }
    Some(HotkeyAction::NextTrack) => {
      media_command(app, crate::music::MediaCommand::Next, "next");
    }
    Some(HotkeyAction::PrevTrack) => {
      media_command(app, crate::music::MediaCommand::Prev, "prev");
    }
    None => {}
  }
}

/// Applies settings changes live by waking the keyboard-hook message thread.
pub fn reload(_app: &tauri::AppHandle, settings: &NotchSettings) {
  #[cfg(target_os = "windows")]
  {
    imp::queue_reload(settings);
  }
  #[cfg(not(target_os = "windows"))]
  let _ = settings;
}

#[cfg(target_os = "windows")]
mod imp {
  use super::*;
  use windows::Win32::Foundation::HWND;
  use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId,
  };
  use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, RegisterHotKey, SetFocus, UnregisterHotKey};
  use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, MSG, PostThreadMessageW,
    SetForegroundWindow, WM_APP, WM_HOTKEY,
  };

  const RELOAD_MESSAGE: u32 = WM_APP + 1;

  pub fn register(action: HotkeyAction, parsed: &ParsedShortcut) -> Result<(), String> {
    let flags = HOT_KEY_MODIFIERS(parsed.modifiers() | MOD_NOREPEAT_BIT);
    unsafe { RegisterHotKey(None, action.id(), flags, parsed.key_code()) }
      .map_err(|err| err.to_string())
  }

  pub fn unregister(action: HotkeyAction) {
    let _ = unsafe { UnregisterHotKey(None, action.id()) };
  }

  /// Called once on the keyboard-hook message thread, before its `GetMessageW`
  /// loop starts. `RegisterHotKey(None, ..)` binds to this thread's queue.
  pub fn start(app: &tauri::AppHandle) {
    THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
    register_all(app, &crate::settings::load_or_default(app).notch);
  }

  /// Stores the newest settings and wakes the message thread. Registration is
  /// always performed on that thread, never from the settings command itself.
  pub fn queue_reload(settings: &NotchSettings) {
    if let Ok(mut guard) = PENDING.lock() {
      *guard = Some(settings.clone());
    }
    let thread_id = THREAD_ID.load(Ordering::Relaxed);
    if thread_id == 0 {
      log::warn!("hotkeys: message thread unavailable for reload");
      return;
    }
    if let Err(err) = unsafe {
      PostThreadMessageW(
        thread_id,
        RELOAD_MESSAGE,
        windows::Win32::Foundation::WPARAM(0),
        windows::Win32::Foundation::LPARAM(0),
      )
    } {
      log::error!("hotkeys: failed to wake message thread: {err}");
    }
  }

  /// Handles `WM_HOTKEY` and the settings reload message on the same thread.
  pub fn on_message(app: &tauri::AppHandle, message: &MSG) {
    if message.message == WM_HOTKEY {
      handle_hotkey(app, message.wParam.0 as i32);
    } else if message.message == RELOAD_MESSAGE {
      let next = match PENDING.lock() {
        Ok(mut guard) => guard.take(),
        Err(poisoned) => poisoned.into_inner().take(),
      };
      if let Some(settings) = next {
        register_all(app, &settings);
      }
    }
  }

  pub fn stop() {
    for action in HotkeyAction::ALL {
      unregister(action);
    }
    THREAD_ID.store(0, Ordering::Relaxed);
    if let Ok(mut guard) = PENDING.lock() {
      *guard = None;
    }
  }

  fn foreground_is(hwnd: HWND) -> bool {
    unsafe { GetForegroundWindow() == hwnd }
  }

  /// Tries Tauri's set_focus first. A global hotkey normally lets Windows
  /// allow the foreground switch; when the webview still lacks keyboard focus,
  /// attach to the foreground thread and repeat the switch.
  pub fn focus_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
      log::error!("hotkey focus: main window not found");
      return;
    };
    let hwnd = match window.hwnd() {
      Ok(hwnd) => hwnd,
      Err(err) => {
        log::error!("hotkey focus: hwnd unavailable ({err})");
        return;
      }
    };

    if window.set_focus().is_ok() && foreground_is(hwnd) {
      log::info!("hotkey open-notes focus: method=set_focus");
      return;
    }

    unsafe {
      let current = GetCurrentThreadId();
      let foreground = GetForegroundWindow();
      let foreground_thread = GetWindowThreadProcessId(foreground, None);
      let attached = if foreground_thread != 0 && foreground_thread != current {
        AttachThreadInput(current, foreground_thread, true).as_bool()
      } else {
        false
      };
      if attached {
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
        let _ = AttachThreadInput(current, foreground_thread, false);
      }
      if foreground_is(hwnd) {
        log::info!("hotkey open-notes focus: method=attach_thread_input");
      } else {
        log::warn!("hotkey open-notes focus: both methods failed");
      }
    }
  }
}

#[cfg(target_os = "windows")]
pub use imp::{on_message, start, stop};

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_bindings_parse() {
    assert_eq!(parse_shortcut("Alt+N").unwrap().canonical(), "Alt + N");
    assert_eq!(parse_shortcut("Alt+C").unwrap().key_code(), 0x43);
    assert_eq!(parse_shortcut("Alt+Z").unwrap().key_code(), 0x5A);
    assert_eq!(parse_shortcut("Alt + N").unwrap().modifiers(), MOD_ALT_BIT);
  }

  #[test]
  fn modifier_is_required() {
    assert!(parse_shortcut("N").is_none());
    assert!(parse_shortcut("Alt").is_none());
    assert!(parse_shortcut("Alt Alt").is_none());
  }

  #[test]
  fn no_repeat_flag_is_always_requested() {
    assert_eq!(MOD_NOREPEAT_BIT, 0x4000);
  }

  #[test]
  fn duplicate_bindings_are_rejected() {
    let mut settings = crate::settings::NotchSettings::default();
    settings.hotkey_next = settings.hotkey_notes.clone();
    let parsed = parsed_bindings(&settings);
    assert!(parsed[0].1.is_some());
    assert!(parsed[1].1.is_none());
    assert!(parsed[2].1.is_some());

    settings.hotkey_next = "Alt+C".to_string();
    settings.shortcut = "Alt + C".to_string();
    let parsed = parsed_bindings(&settings);
    assert!(parsed[1].1.is_none());
  }
}
