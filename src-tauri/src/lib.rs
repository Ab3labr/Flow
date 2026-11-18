use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager};

mod triggers;
mod window_mode;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      settings_get,
      settings_set,
      window_mode::set_ui_mode
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

      // Startup mode is idle: hidden and click-through.
      if let Err(err) = window_mode::apply(&window, window_mode::UiMode::Idle, 0.0) {
        log::warn!("failed to apply startup idle mode: {err}");
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.hide();
      }

      let handle = app.handle();
      let open_item = MenuItem::with_id(handle, "open-float", "Open Float", true, None::<&str>)?;
      let settings_item = MenuItem::with_id(handle, "settings", "Settings", true, None::<&str>)?;
      let quit_item = MenuItem::with_id(handle, "quit", "Quit", true, None::<&str>)?;
      let menu = Menu::with_items(handle, &[&open_item, &settings_item, &quit_item])?;

      let mut tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
          "open-float" => {
            if let Err(err) = app.emit("open-request", ()) {
              log::warn!("failed to emit open-request: {err}");
            }
          }
          "settings" => {
            if let Err(err) = app.emit("open-request", serde_json::json!({ "tab": "settings" })) {
              log::warn!("failed to emit open-request: {err}");
            }
          }
          "quit" => app.exit(0),
          _ => {}
        });
      if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
      }
      tray.build(app)?;

      #[cfg(target_os = "windows")]
      {
        let handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
          settings::ensure_settings_file(&handle).await;
          triggers::start(handle.clone());
          triggers::start_alt_hook(handle);
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
  use tauri::Manager;

  fn default_true() -> bool {
    true
  }

  fn default_shortcut() -> String {
    "Alt Alt".to_string()
  }

  /// Shared with the frontend: the same camelCase names, and every
  /// `#[serde(default)]` below keeps a partial settings.json usable. Old keys
  /// from earlier versions ("fullscreen", "launch") are simply ignored.
  #[derive(Debug, Serialize, Deserialize, Default)]
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
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
  }

  #[derive(Debug, Serialize, Deserialize, Default)]
  #[serde(rename_all = "camelCase")]
  pub struct AppSettings {
    pub notch: NotchSettings,
    pub notes_folder: Option<String>,
    pub calendar_accounts: Vec<CalendarAccountMeta>,
    pub timer_custom_times: Vec<u32>,
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
    "notch": {
      "topEdge": true,
      "altAlt": true,
      "hideInFullscreen": true,
      "launchAtStartup": true,
      "upcomingEvent": true,
      "noCalendars": false,
      "shortcut": "Alt Alt"
    },
    "notesFolder": null,
    "calendarAccounts": [],
    "timerCustomTimes": []
  }"#;

  pub async fn ensure_settings_file(handle: &tauri::AppHandle) {
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
    let config_dir = handle.path().app_config_dir().map_err(|e| e.to_string())?;
    let settings_path = config_dir.join("settings.json");
    let temp_path = config_dir.join("settings.json.tmp");
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&temp_path, content).map_err(|e| e.to_string())?;
    fs::rename(&temp_path, &settings_path).map_err(|e| e.to_string())?;
    Ok(())
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
    fn defaults_json_matches_frontend_defaults() {
      let notch: NotchSettings =
        serde_json::from_str::<AppSettings>(DEFAULT_SETTINGS).unwrap().notch;
      assert!(notch.top_edge);
      assert!(notch.alt_alt);
      assert!(notch.hide_in_fullscreen);
      assert!(notch.launch_at_startup);
      assert!(notch.upcoming_event);
      assert!(!notch.no_calendars);
      assert_eq!(notch.shortcut, "Alt Alt");
    }
  }
}

use settings::AppSettings;

#[tauri::command]
async fn settings_get(handle: tauri::AppHandle) -> Result<AppSettings, String> {
  settings::load_settings(&handle)
}

#[tauri::command]
async fn settings_set(handle: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
  settings::save_settings(&handle, &settings)
}

