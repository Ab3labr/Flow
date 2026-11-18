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

      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

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
          triggers::start(handle);
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

  #[derive(Debug, Serialize, Deserialize, Default)]
  #[serde(rename_all = "camelCase")]
  pub struct NotchSettings {
    pub top_edge: bool,
    pub alt_alt: bool,
    pub hide_in_fullscreen: bool,
    pub launch_at_startup: bool,
    pub upcoming_event: bool,
    pub no_calendars: bool,
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
      "launchAtStartup": false,
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

