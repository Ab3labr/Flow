use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;

mod music;
mod power;
mod triggers;
mod window_mode;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      settings_get,
      settings_set,
      music::media_snapshot,
      music::media_toggle,
      music::media_next,
      music::media_prev,
      music::media_seek,
      window_mode::set_ui_mode,
      window_mode::ui_ready,
      fullscreen_active
    ])
    .setup(|app| {
      #[cfg(target_os = "windows")]
      {
        use windows::Win32::System::Threading::CreateMutexW;
        use windows::core::PCWSTR;

        let mutex_name: Vec<u16> = "com.float.app.windows\0".encode_utf16().collect();
        let _mutex = unsafe { CreateMutexW(None, true, PCWSTR(mutex_name.as_ptr())) };

        let last_error = unsafe { windows::Win32::Foundation::GetLastError() };
        if last_error.0 == 183 {
          eprintln!("Float is already running");
          std::process::exit(0);
        }
      }

      // Files land in the app log dir as float.log. Kept on in release builds:
      // the Alt hook writes one line per key event for field diagnosis.
      app.handle().plugin(
        tauri_plugin_log::Builder::default()
          .level(log::LevelFilter::Info)
          .clear_targets()
          .target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout))
          .target(tauri_plugin_log::Target::new(
            tauri_plugin_log::TargetKind::LogDir {
              file_name: Some("float".into()),
            },
          ))
          .build(),
      )?;

      let window = app.get_webview_window("main").unwrap();

      // Rust owns open/pinned/rect for the close triggers.
      app.manage(window_mode::AppState::default());

      // Startup mode is idle: hidden and click-through. The webview is not
      // allowed to open anything until it calls `ui_ready` (see ui_ready).
      {
        let state = app.state::<window_mode::AppState>();
        if let Err(err) = window_mode::apply(
          &window,
          state.inner(),
          window_mode::UiMode::Idle,
          window_mode::UiPhase::Stable,
          0.0,
          0.0,
          0.0,
          false,
          false,
        ) {
          log::warn!("failed to apply startup idle mode: {err}");
          let _ = window.set_ignore_cursor_events(true);
          let _ = window.hide();
        }
        if music::debug_enabled() {
          log::info!(
            "[window] startup: mode=idle phase=stable logical_open=false ui_ready=false window_visible={} ignore_cursor=true",
            window.is_visible().unwrap_or(false)
          );
        }
      }

      let handle = app.handle();
      let open_item = MenuItem::with_id(handle, "open-float", "Open Float", true, None::<&str>)?;
      let settings_item = MenuItem::with_id(handle, "settings", "Settings", true, None::<&str>)?;
      let quit_item = MenuItem::with_id(handle, "quit", "Quit", true, None::<&str>)?;
      let menu = Menu::with_items(handle, &[&open_item, &settings_item, &quit_item])?;

      let mut tray = TrayIconBuilder::with_id("main")
        // Updated to "Float" by the ui_ready handshake (window_mode::ui_ready).
        .tooltip("Float (starting...)")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
          "open-float" => {
            window_mode::emit_open_request(
              app,
              serde_json::json!({ "source": "tray", "pinned": true }),
            );
          }
          "settings" => {
            window_mode::emit_open_request(
              app,
              serde_json::json!({ "tab": "settings", "source": "tray", "pinned": true }),
            );
          }
          "quit" => app.exit(0),
          _ => {}
        });
      if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
      }
      tray.build(app)?;

      // Seed the live settings before any trigger thread starts.
      settings::ensure_settings_file(app.handle());
      settings::init(app.handle());

      // SMTC snapshot: one MTA thread, cached for `media_snapshot`, emitted as
      // `media-state` only when the track/status changes.
      app.manage(std::sync::Arc::new(music::MediaState::default()));
      music::start(app.handle().clone());

      // AC/battery changes are emitted as `power-state`; startup is silent.
      power::start(app.handle().clone());

      #[cfg(target_os = "windows")]
      {
        let handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
          triggers::start(handle.clone());
          triggers::start_alt_hook(handle.clone());
          triggers::start_mouse_hook(handle);
        });
      }

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}

mod settings {
  use serde::{Deserialize, Serialize};
  use std::fs;
  use std::sync::{LazyLock, RwLock};
  use tauri::Manager;

  /// Bump when a release must reset the four live toggles once.
  pub const SETTINGS_VERSION: u32 = 2;

  fn default_true() -> bool {
    true
  }

  fn default_shortcut() -> String {
    "Alt Alt".to_string()
  }

  /// Shared with the frontend: the same camelCase names, and every
  /// `#[serde(default)]` below keeps a partial settings.json usable. Old keys
  /// from earlier versions ("fullscreen", "launch") are simply ignored.
  #[derive(Debug, Clone, Serialize, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct NotchSettings {
    #[serde(default = "default_true")]
    pub top_edge: bool,
    #[serde(default = "default_true")]
    pub alt_alt: bool,
    #[serde(default = "default_true")]
    pub hide_in_fullscreen: bool,
    #[serde(default = "default_true")]
    pub launch_at_startup: bool,
    #[serde(default = "default_true")]
    pub upcoming_event: bool,
    #[serde(default)]
    pub no_calendars: bool,
    #[serde(default = "default_true")]
    pub compact_bar: bool,
    #[serde(default = "default_true")]
    pub compact_music: bool,
    #[serde(default = "default_true")]
    pub compact_timer: bool,
    #[serde(default = "default_true")]
    pub compact_recording: bool,
    #[serde(default = "default_true")]
    pub compact_charging: bool,
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
  }

  /// Must agree with the serde defaults above: `#[derive(Default)]` would turn
  /// every bool into false.
  impl Default for NotchSettings {
    fn default() -> Self {
      Self {
        top_edge: true,
        alt_alt: true,
        hide_in_fullscreen: true,
        launch_at_startup: true,
        upcoming_event: true,
        no_calendars: false,
        compact_bar: true,
        compact_music: true,
        compact_timer: true,
        compact_recording: true,
        compact_charging: true,
        shortcut: default_shortcut(),
      }
    }
  }

  #[derive(Debug, Serialize, Deserialize, Default)]
  #[serde(rename_all = "camelCase")]
  pub struct AppSettings {
    /// 0 when the field is missing from an older file, which triggers the
    /// one-time migration in `apply_version_migration`.
    #[serde(default)]
    pub settings_version: u32,
    #[serde(default)]
    pub notch: NotchSettings,
    #[serde(default)]
    pub notes_folder: Option<String>,
    #[serde(default)]
    pub calendar_accounts: Vec<CalendarAccountMeta>,
    #[serde(default)]
    pub timer_custom_times: Vec<u32>,
  }

  /// Live settings read by the trigger threads every cycle. `settings_set`
  /// replaces the contents immediately, so toggles apply without a restart.
  static LIVE: LazyLock<RwLock<NotchSettings>> =
    LazyLock::new(|| RwLock::new(NotchSettings::default()));

  fn read_live<R>(f: impl FnOnce(&NotchSettings) -> R) -> R {
    match LIVE.read() {
      Ok(guard) => f(&guard),
      Err(poisoned) => f(&poisoned.into_inner()),
    }
  }

  /// True when the top-edge hover zone is enabled.
  pub fn top_edge() -> bool {
    read_live(|notch| notch.top_edge)
  }

  /// True when double-Alt is enabled.
  pub fn alt_alt() -> bool {
    read_live(|notch| notch.alt_alt)
  }

  /// True when hover is suppressed by fullscreen apps.
  pub fn hide_in_fullscreen() -> bool {
    read_live(|notch| notch.hide_in_fullscreen)
  }

  pub fn set_live(notch: &NotchSettings) {
    let mut guard = match LIVE.write() {
      Ok(guard) => guard,
      Err(poisoned) => poisoned.into_inner(),
    };
    *guard = notch.clone();
  }

  pub fn log_values(notch: &NotchSettings) {
    log::info!(
      "settings: topEdge={} altAlt={} hideInFullscreen={} launchAtStartup={}",
      notch.top_edge,
      notch.alt_alt,
      notch.hide_in_fullscreen,
      notch.launch_at_startup
    );
  }

  #[derive(Debug, Serialize, Deserialize, Clone)]
  #[serde(rename_all = "camelCase")]
  pub struct CalendarAccountMeta {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub email: String,
    pub color: String,
    pub visible: bool,
    pub status: String,
  }

  const DEFAULT_SETTINGS: &str = r#"{
    "settingsVersion": 2,
    "notch": {
      "topEdge": true,
      "altAlt": true,
      "hideInFullscreen": true,
      "launchAtStartup": true,
      "upcomingEvent": true,
      "noCalendars": false,
      "compactBar": true,
      "compactMusic": true,
      "compactTimer": true,
      "compactRecording": true,
      "compactCharging": true,
      "shortcut": "Alt Alt"
    },
    "notesFolder": null,
    "calendarAccounts": [],
    "timerCustomTimes": []
  }"#;

  pub fn ensure_settings_file(handle: &tauri::AppHandle) {
    let config_dir = handle.path().app_config_dir().expect("Failed to get config dir");
    fs::create_dir_all(&config_dir).expect("Failed to create config dir");

    let settings_path = config_dir.join("settings.json");
    if !settings_path.exists() {
      fs::write(&settings_path, DEFAULT_SETTINGS).expect("Failed to write default settings");
    }
  }

  #[allow(dead_code)]
  pub fn load_settings(handle: &tauri::AppHandle) -> Result<AppSettings, String> {
    let config_dir = handle.path().app_config_dir().map_err(|e| e.to_string())?;
    let settings_path = config_dir.join("settings.json");
    let content = fs::read_to_string(&settings_path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
  }

  #[allow(dead_code)]
  pub fn save_settings(handle: &tauri::AppHandle, settings: &AppSettings) -> Result<(), String> {
    // Two quick toggles can run `settings_set` concurrently; the shared temp
    // file would otherwise race.
    static SAVE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = SAVE_LOCK
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    let config_dir = handle.path().app_config_dir().map_err(|e| e.to_string())?;
    let settings_path = config_dir.join("settings.json");
    let temp_path = config_dir.join("settings.json.tmp");
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&temp_path, content).map_err(|e| e.to_string())?;
    fs::rename(&temp_path, &settings_path).map_err(|e| e.to_string())?;
    Ok(())
  }

  /// Reads the file, or falls back to defaults when it is missing/corrupt.
  pub fn load_or_default(handle: &tauri::AppHandle) -> AppSettings {
    match load_settings(handle) {
      Ok(settings) => settings,
      Err(err) => {
        log::warn!("settings: using defaults ({err})");
        AppSettings::default()
      }
    }
  }

  /// Missing (0) or older versions reset the four live toggles to true once.
  /// Returns true when the file needs to be written back. Pure so it can be
  /// unit-tested without touching disk.
  fn apply_version_migration(settings: &mut AppSettings) -> bool {
    if settings.settings_version >= SETTINGS_VERSION {
      return false;
    }
    settings.notch.top_edge = true;
    settings.notch.alt_alt = true;
    settings.notch.hide_in_fullscreen = true;
    settings.notch.launch_at_startup = true;
    settings.settings_version = SETTINGS_VERSION;
    true
  }

  /// Loads once at startup: migrate if needed, seed the live state, log the
  /// four toggles. Missing/corrupt files end up at the defaults.
  pub fn init(handle: &tauri::AppHandle) {
    let mut settings = load_or_default(handle);
    if apply_version_migration(&mut settings) {
      if let Err(err) = save_settings(handle, &settings) {
        log::warn!("settings: failed to save migrated file ({err})");
      }
    }
    set_live(&settings.notch);
    log_values(&settings.notch);
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    #[test]
    fn partial_notch_settings_use_defaults() {
      let notch: NotchSettings = serde_json::from_str("{}").unwrap();
      assert!(notch.top_edge);
      assert!(notch.alt_alt);
      assert!(notch.hide_in_fullscreen);
      assert!(notch.launch_at_startup);
      assert!(notch.upcoming_event);
      assert!(!notch.no_calendars);
      assert!(notch.compact_bar);
      assert!(notch.compact_music);
      assert!(notch.compact_timer);
      assert!(notch.compact_recording);
      assert!(notch.compact_charging);
      assert_eq!(notch.shortcut, "Alt Alt");
    }

    #[test]
    fn missing_toggles_default_to_true() {
      let notch: NotchSettings =
        serde_json::from_str(r#"{ "shortcut": "Ctrl + Shift + Space" }"#).unwrap();
      assert!(notch.top_edge);
      assert!(notch.alt_alt);
      assert!(notch.hide_in_fullscreen);
      assert!(notch.launch_at_startup);
      assert!(notch.compact_bar);
      assert!(notch.compact_music);
      assert!(notch.compact_timer);
      assert!(notch.compact_recording);
      assert!(notch.compact_charging);
      assert_eq!(notch.shortcut, "Ctrl + Shift + Space");
    }

    #[test]
    fn legacy_keys_are_ignored() {
      let notch: NotchSettings =
        serde_json::from_str(r#"{ "fullscreen": false, "launch": false }"#).unwrap();
      assert!(notch.hide_in_fullscreen);
      assert!(notch.launch_at_startup);
    }

    #[test]
    fn notch_settings_default_is_all_true() {
      let notch = NotchSettings::default();
      assert!(notch.top_edge);
      assert!(notch.alt_alt);
      assert!(notch.hide_in_fullscreen);
      assert!(notch.launch_at_startup);
      assert!(notch.upcoming_event);
      assert!(!notch.no_calendars);
      assert!(notch.compact_bar);
      assert!(notch.compact_music);
      assert!(notch.compact_timer);
      assert!(notch.compact_recording);
      assert!(notch.compact_charging);
      assert_eq!(notch.shortcut, "Alt Alt");
    }

    #[test]
    fn missing_version_migrates_and_resets_the_four_toggles() {
      let mut settings: AppSettings = serde_json::from_str(
        r#"{ "notch": {
          "topEdge": false,
          "altAlt": false,
          "hideInFullscreen": false,
          "launchAtStartup": false,
          "noCalendars": true
        } }"#,
      )
      .unwrap();
      assert_eq!(settings.settings_version, 0);
      assert!(apply_version_migration(&mut settings));
      assert_eq!(settings.settings_version, SETTINGS_VERSION);
      assert!(settings.notch.top_edge);
      assert!(settings.notch.alt_alt);
      assert!(settings.notch.hide_in_fullscreen);
      assert!(settings.notch.launch_at_startup);
      // Unrelated values are untouched, and migration only runs once.
      assert!(settings.notch.no_calendars);
      assert!(!apply_version_migration(&mut settings));
    }

    #[test]
    fn current_version_keeps_saved_toggles() {
      let mut settings: AppSettings = serde_json::from_str(
        r#"{ "settingsVersion": 2, "notch": {
          "topEdge": false,
          "altAlt": false,
          "hideInFullscreen": false,
          "launchAtStartup": false
        } }"#,
      )
      .unwrap();
      assert!(!apply_version_migration(&mut settings));
      assert!(!settings.notch.top_edge);
      assert!(!settings.notch.alt_alt);
      assert!(!settings.notch.hide_in_fullscreen);
      assert!(!settings.notch.launch_at_startup);
    }

    #[test]
    fn empty_object_loads_all_defaults() {
      let settings: AppSettings = serde_json::from_str("{}").unwrap();
      assert_eq!(settings.settings_version, 0);
      assert!(settings.notch.top_edge);
      assert!(settings.notes_folder.is_none());
      assert!(settings.calendar_accounts.is_empty());
      assert!(settings.timer_custom_times.is_empty());
    }

    #[test]
    fn defaults_json_matches_frontend_defaults() {
      let settings = serde_json::from_str::<AppSettings>(DEFAULT_SETTINGS).unwrap();
      assert_eq!(settings.settings_version, SETTINGS_VERSION);
      let notch = settings.notch;
      assert!(notch.top_edge);
      assert!(notch.alt_alt);
      assert!(notch.hide_in_fullscreen);
      assert!(notch.launch_at_startup);
      assert!(notch.upcoming_event);
      assert!(!notch.no_calendars);
      assert!(notch.compact_bar);
      assert!(notch.compact_music);
      assert!(notch.compact_timer);
      assert!(notch.compact_recording);
      assert!(notch.compact_charging);
      assert_eq!(notch.shortcut, "Alt Alt");
    }
  }
}

use settings::AppSettings;

/// True while a fullscreen app is in front; the compact bar hides when the
/// `hideInFullscreen` setting is on.
#[tauri::command]
fn fullscreen_active() -> bool {
  triggers::foreground_fullscreen_active()
}

#[tauri::command]
async fn settings_get(handle: tauri::AppHandle) -> Result<AppSettings, String> {
  let settings = settings::load_or_default(&handle);
  settings::set_live(&settings.notch);
  Ok(settings)
}

#[tauri::command]
async fn settings_set(
  handle: tauri::AppHandle,
  mut settings: AppSettings,
) -> Result<(), String> {
  // Never let a frontend payload downgrade the version and re-trigger the
  // one-time migration.
  settings.settings_version = settings::SETTINGS_VERSION;
  settings::save_settings(&handle, &settings)?;
  settings::set_live(&settings.notch);
  settings::log_values(&settings.notch);
  Ok(())
}

