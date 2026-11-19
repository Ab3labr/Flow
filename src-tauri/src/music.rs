//! Now playing from Windows SMTC (Phase 3A).
//!
//! One dedicated thread owns the WinRT apartment, the session manager and the
//! album-art cache. The rest of the app only reads the cached snapshot (or
//! listens to `media-state`), so neither the UI thread nor the webview ever
//! blocks on a WinRT `.join()`.

#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Manager;

/// Poll cadence for the SMTC thread.
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Album art larger than this is skipped (base64 would bloat the event).
pub const MAX_ART_BYTES: u64 = 1_500_000;

/// What the frontend receives: `media_snapshot` + `media-state` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSnapshot {
  pub title: String,
  pub artist: String,
  pub source: String,
  pub playing: bool,
  pub position_ms: i64,
  pub duration_ms: Option<i64>,
  pub art_data_url: Option<String>,
  pub captured_at_ms: i64,
}

impl MediaSnapshot {
  /// The fields that make a *new* track/state worth emitting. Position ticks
  /// are intentionally excluded; the UI interpolates those locally.
  pub fn same_state(&self, other: &Self) -> bool {
    self.title == other.title
      && self.artist == other.artist
      && self.source == other.source
      && self.playing == other.playing
      && self.duration_ms == other.duration_ms
  }
}

#[derive(Default)]
struct Cache {
  current: Option<MediaSnapshot>,
  initialized: bool,
}

/// Change tracking for the poll thread: `media-state` is emitted only when
/// this moves between "no session" and a new track/status.
pub enum LastState {
  Uninitialized,
  NoSession,
  Session(MediaSnapshot),
}

impl LastState {
  pub fn changed(&self, next: &Option<MediaSnapshot>) -> bool {
    match (self, next) {
      (LastState::Uninitialized, _) => true,
      (LastState::NoSession, None) => false,
      (LastState::NoSession, Some(_)) => true,
      (LastState::Session(_), None) => true,
      (LastState::Session(previous), Some(current)) => !previous.same_state(current),
    }
  }
}

/// Rust-owned latest snapshot, shared with the `media_snapshot` command.
#[derive(Default)]
pub struct MediaState {
  cache: Mutex<Cache>,
  ready: Condvar,
}

impl MediaState {
  fn lock(&self) -> std::sync::MutexGuard<'_, Cache> {
    match self.cache.lock() {
      Ok(guard) => guard,
      Err(poisoned) => poisoned.into_inner(),
    }
  }

  fn publish(&self, snapshot: Option<MediaSnapshot>) {
    let mut cache = self.lock();
    cache.current = snapshot;
    cache.initialized = true;
    self.ready.notify_all();
  }

  /// Reads the latest snapshot, waiting up to `timeout` for the first poll.
  /// Called from a blocking task, never from the UI thread.
  pub fn wait_snapshot(&self, timeout: Duration) -> Option<MediaSnapshot> {
    let mut cache = self.lock();
    if !cache.initialized {
      cache = match self.ready.wait_timeout(cache, timeout) {
        Ok((guard, _)) => guard,
        Err(poisoned) => poisoned.into_inner().0,
      };
    }
    cache.current.clone()
  }
}

/// Returns the cached now-playing state. `None` means "no session" (idle home).
#[tauri::command]
pub async fn media_snapshot(
  state: tauri::State<'_, Arc<MediaState>>,
) -> Result<Option<MediaSnapshot>, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || state.wait_snapshot(Duration::from_millis(2_000)))
    .await
    .map_err(|err| err.to_string())
}

/// Starts the single SMTC polling thread.
pub fn start(app: tauri::AppHandle) {
  let shared = app.state::<Arc<MediaState>>().inner().clone();

  #[cfg(target_os = "windows")]
  imp::start(app, shared);

  #[cfg(not(target_os = "windows"))]
  {
    let _ = app;
    // No WinRT off Windows: report "no session" once and let the app run.
    shared.publish(None);
  }
}

/// `TimeSpan::Duration` is in 100 ns units.
pub fn track_duration_ms(start_100ns: i64, end_100ns: i64) -> Option<i64> {
  let delta = end_100ns.saturating_sub(start_100ns);
  if delta <= 0 {
    None
  } else {
    Some(delta / 10_000)
  }
}

pub fn ticks_to_ms(ticks_100ns: i64) -> i64 {
  (ticks_100ns / 10_000).max(0)
}

/// Maps a Windows app user-model ID to the label shown in the card.
pub fn source_label(app_id: &str) -> String {
  let lower = app_id.to_ascii_lowercase();
  let known = [
    ("spotify", "Spotify"),
    ("applemusic", "Apple Music"),
    ("itunes", "Apple Music"),
    ("chrome", "Chrome"),
    ("msedge", "Edge"),
    ("brave", "Brave"),
    ("firefox", "Firefox"),
    ("opera", "Opera"),
  ];
  for (needle, label) in known {
    if lower.contains(needle) {
      return label.to_string();
    }
  }
  readable_app_id(app_id)
}

/// Fallback label: the human-readable part of an AUMID / executable path.
fn readable_app_id(app_id: &str) -> String {
  let trimmed = app_id.trim();
  if trimmed.is_empty() {
    return "Unknown".to_string();
  }
  let file = trimmed
    .rsplit(|character| character == '\\' || character == '/')
    .next()
    .unwrap_or(trimmed);
  let after_bang = file.rsplit('!').next().unwrap_or(file);
  let no_ext = after_bang
    .strip_suffix(".exe")
    .or_else(|| after_bang.strip_suffix(".EXE"))
    .unwrap_or(after_bang);
  let base = no_ext.split('_').next().unwrap_or(no_ext);
  if base.is_empty() {
    trimmed.to_string()
  } else {
    base.to_string()
  }
}

const BASE64_ALPHABET: &[u8; 64] =
  b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Tiny base64 encoder (no new dependency for one data URL).
pub fn base64_encode(input: &[u8]) -> String {
  let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
  for chunk in input.chunks(3) {
    let first = chunk[0] as u32;
    let second = *chunk.get(1).unwrap_or(&0) as u32;
    let third = *chunk.get(2).unwrap_or(&0) as u32;
    let combined = (first << 16) | (second << 8) | third;

    output.push(BASE64_ALPHABET[((combined >> 18) & 63) as usize] as char);
    output.push(BASE64_ALPHABET[((combined >> 12) & 63) as usize] as char);
    output.push(if chunk.len() > 1 {
      BASE64_ALPHABET[((combined >> 6) & 63) as usize] as char
    } else {
      '='
    });
    output.push(if chunk.len() > 2 {
      BASE64_ALPHABET[(combined & 63) as usize] as char
    } else {
      '='
    });
  }
  output
}

fn now_ms() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|duration| duration.as_millis() as i64)
    .unwrap_or(0)
}

#[cfg(target_os = "windows")]
mod imp {
  use super::{
    base64_encode, now_ms, source_label, ticks_to_ms, track_duration_ms, LastState, MediaSnapshot,
    MediaState, MAX_ART_BYTES, POLL_INTERVAL,
  };
  use std::sync::Arc;
  use tauri::Emitter;
  use windows::core::Result as WinResult;
  use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionMediaProperties,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
  };
  use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};
  use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

  type Manager = GlobalSystemMediaTransportControlsSessionManager;
  type Session = GlobalSystemMediaTransportControlsSession;

  #[derive(Default)]
  struct ArtCache {
    key: Option<(String, String)>,
    url: Option<String>,
  }

  struct Polled {
    app_id: Option<String>,
    snapshot: Option<MediaSnapshot>,
  }

  pub fn start(app: tauri::AppHandle, shared: Arc<MediaState>) {
    let spawned = std::thread::Builder::new()
      .name("float-media".into())
      .spawn(move || run(app, shared));

    if let Err(err) = spawned {
      log::error!("media: failed to start poll thread: {err}");
    }
  }

  fn run(app: tauri::AppHandle, shared: Arc<MediaState>) {
    let debug = std::env::var("FLOAT_DEBUG").ok().as_deref() == Some("1");

    // SAFETY: this is the first WinRT call on this dedicated thread.
    if let Err(err) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
      log::error!("media: RoInitialize(MTA) failed: {err}");
      shared.publish(None);
      return;
    }

    log::info!(
      "media: SMTC polling every {} ms",
      POLL_INTERVAL.as_millis()
    );

    let mut manager: Option<Manager> = None;
    let mut art = ArtCache::default();
    let mut last = LastState::Uninitialized;
    let mut last_app_id: Option<String> = None;
    let mut manager_error_logged = false;
    let mut session_error_logged = false;

    loop {
      let polled = poll_once(
        &mut manager,
        &mut art,
        &mut manager_error_logged,
        &mut session_error_logged,
      );

      if debug {
        match &polled.app_id {
          Some(app_id) if last_app_id.as_deref() != Some(app_id) => {
            log::info!("media: chosen session {app_id}");
            last_app_id = Some(app_id.clone());
          }
          None => last_app_id = None,
          _ => {}
        }
      }

      if last.changed(&polled.snapshot) {
        match &polled.snapshot {
          Some(snapshot) => log::info!(
            "media: state -> {} / {} [{}] playing={} position={}ms duration={:?}ms",
            snapshot.title,
            snapshot.artist,
            snapshot.source,
            snapshot.playing,
            snapshot.position_ms,
            snapshot.duration_ms
          ),
          None => log::info!("media: state -> no session"),
        }

        if let Err(err) = app.emit("media-state", polled.snapshot.clone()) {
          log::warn!("media: failed to emit media-state: {err}");
        }
        last = match polled.snapshot.clone() {
          Some(snapshot) => LastState::Session(snapshot),
          None => LastState::NoSession,
        };
      }

      shared.publish(polled.snapshot);
      std::thread::sleep(POLL_INTERVAL);
    }
  }

  fn poll_once(
    manager: &mut Option<Manager>,
    art: &mut ArtCache,
    manager_error_logged: &mut bool,
    session_error_logged: &mut bool,
  ) -> Polled {
    if manager.is_none() {
      match Manager::RequestAsync().and_then(|operation| operation.join()) {
        Ok(value) => {
          *manager = Some(value);
          *manager_error_logged = false;
        }
        Err(err) => {
          if !*manager_error_logged {
            log::warn!("media: session manager unavailable: {err}");
            *manager_error_logged = true;
          }
          return Polled {
            app_id: None,
            snapshot: None,
          };
        }
      }
    }

    let Some(manager) = manager.as_ref() else {
      return Polled {
        app_id: None,
        snapshot: None,
      };
    };

    let session = match choose_session(manager) {
      Ok(Some(session)) => session,
      Ok(None) => {
        *session_error_logged = false;
        return Polled {
          app_id: None,
          snapshot: None,
        };
      }
      Err(err) => {
        if !*session_error_logged {
          log::warn!("media: could not choose a session: {err}");
          *session_error_logged = true;
        }
        return Polled {
          app_id: None,
          snapshot: None,
        };
      }
    };

    let app_id = session
      .SourceAppUserModelId()
      .map(|id| id.to_string())
      .ok();

    match read_snapshot(&session, art) {
      Ok(snapshot) => {
        *session_error_logged = false;
        Polled {
          app_id,
          snapshot: Some(snapshot),
        }
      }
      Err(err) => {
        if !*session_error_logged {
          log::warn!("media: session read failed: {err}");
          *session_error_logged = true;
        }
        Polled {
          app_id,
          snapshot: None,
        }
      }
    }
  }

  fn choose_session(manager: &Manager) -> WinResult<Option<Session>> {
    let sessions = manager.GetSessions()?;
    let count = sessions.Size()?;
    for index in 0..count {
      let session = sessions.GetAt(index)?;
      let playback = match session.GetPlaybackInfo() {
        Ok(playback) => playback,
        Err(_) => continue,
      };
      match playback.PlaybackStatus() {
        Ok(status) if status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => {
          return Ok(Some(session));
        }
        _ => {}
      }
    }

    // Nothing is playing: fall back to the system's current session (it may be
    // paused). A missing session is a valid "no music" state, not an error.
    Ok(manager.GetCurrentSession().ok())
  }

  fn read_snapshot(session: &Session, art: &mut ArtCache) -> WinResult<MediaSnapshot> {
    let app_id = session.SourceAppUserModelId()?.to_string();
    let properties = session.TryGetMediaPropertiesAsync()?.join()?;
    let title = properties.Title()?.to_string();
    let artist = properties.Artist()?.to_string();

    let playback = session.GetPlaybackInfo()?;
    let status = playback.PlaybackStatus()?;
    let playing = status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;

    let timeline = session.GetTimelineProperties()?;
    let start = timeline.StartTime()?.Duration;
    let end = timeline.EndTime()?.Duration;
    let position = timeline.Position()?.Duration;

    let art_data_url = art_for(&properties, &title, &artist, art);

    Ok(MediaSnapshot {
      title,
      artist,
      source: source_label(&app_id),
      playing,
      position_ms: ticks_to_ms(position),
      duration_ms: track_duration_ms(start, end),
      art_data_url,
      captured_at_ms: now_ms(),
    })
  }

  fn art_for(
    properties: &GlobalSystemMediaTransportControlsSessionMediaProperties,
    title: &str,
    artist: &str,
    cache: &mut ArtCache,
  ) -> Option<String> {
    let key = (title.to_string(), artist.to_string());
    if cache.key.as_ref() == Some(&key) {
      return cache.url.clone();
    }

    // Only re-read the thumbnail when the track (title/artist) changes.
    let url = properties
      .Thumbnail()
      .ok()
      .and_then(|reference| thumbnail_data_url(&reference));
    cache.key = Some(key);
    cache.url = url.clone();
    url
  }

  fn thumbnail_data_url(reference: &IRandomAccessStreamReference) -> Option<String> {
    let stream = reference.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()?;
    if size == 0 || size > MAX_ART_BYTES {
      return None;
    }

    let content_type = stream
      .ContentType()
      .map(|value| value.to_string())
      .unwrap_or_default();
    let input = stream.GetInputStreamAt(0).ok()?;
    let reader = DataReader::CreateDataReader(&input).ok()?;
    let loaded = reader.LoadAsync(size as u32).ok()?.join().ok()?;
    if loaded == 0 {
      return None;
    }

    let mut bytes = vec![0u8; loaded as usize];
    reader.ReadBytes(&mut bytes).ok()?;

    let mime = content_type
      .split(';')
      .next()
      .map(str::trim)
      .filter(|value| !value.is_empty())
      .unwrap_or("image/jpeg");
    Some(format!("data:{mime};base64,{}", base64_encode(&bytes)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn base64_matches_rfc4648_vectors() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
  }

  #[test]
  fn duration_is_null_when_end_is_not_after_start() {
    assert_eq!(track_duration_ms(0, 0), None);
    assert_eq!(track_duration_ms(10, 10), None);
    assert_eq!(track_duration_ms(10, 5), None);
    assert_eq!(track_duration_ms(0, 2_420_000_000), Some(242_000));
    assert_eq!(track_duration_ms(1_000_000, 2_421_000_000), Some(242_000));
  }

  #[test]
  fn ticks_are_converted_from_100ns_and_clamped() {
    assert_eq!(ticks_to_ms(10_000), 1);
    assert_eq!(ticks_to_ms(1_080_000_000), 108_000);
    assert_eq!(ticks_to_ms(-10_000), 0);
  }

  #[test]
  fn known_sources_are_labelled_case_insensitively() {
    assert_eq!(source_label("Spotify.exe"), "Spotify");
    assert_eq!(source_label("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"), "Spotify");
    assert_eq!(source_label("AppleMusic.exe"), "Apple Music");
    assert_eq!(source_label("iTunes.exe"), "Apple Music");
    assert_eq!(source_label("Google.Chrome"), "Chrome");
    assert_eq!(source_label("MSEdge.exe"), "Edge");
    assert_eq!(source_label("BraveSoftware.Brave"), "Brave");
    assert_eq!(source_label("firefox.exe"), "Firefox");
    assert_eq!(source_label("Opera.exe"), "Opera");
  }

  #[test]
  fn unknown_source_falls_back_to_the_readable_id() {
    assert_eq!(
      source_label("C:\\Program Files\\VLC\\vlc.exe"),
      "vlc"
    );
    assert_eq!(
      source_label("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"),
      "Microsoft.ZuneMusic"
    );
    assert_eq!(source_label(""), "Unknown");
  }

  #[test]
  fn only_track_state_changes_count_as_new_state() {
    let base = MediaSnapshot {
      title: "Song".into(),
      artist: "Artist".into(),
      source: "Spotify".into(),
      playing: true,
      position_ms: 1_000,
      duration_ms: Some(200_000),
      art_data_url: None,
      captured_at_ms: 10,
    };

    let mut ticked = base.clone();
    ticked.position_ms = 2_000;
    ticked.captured_at_ms = 20;
    assert!(base.same_state(&ticked));

    let mut painted = base.clone();
    painted.art_data_url = Some("data:image/png;base64,AA==".into());
    assert!(base.same_state(&painted));

    let mut paused = base.clone();
    paused.playing = false;
    assert!(!base.same_state(&paused));

    let mut renamed = base.clone();
    renamed.title = "Other".into();
    assert!(!base.same_state(&renamed));

    let mut resized = base.clone();
    resized.duration_ms = None;
    assert!(!base.same_state(&resized));
  }

  #[test]
  fn last_state_emits_only_on_track_changes() {
    let snapshot = MediaSnapshot {
      title: "Song".into(),
      artist: "Artist".into(),
      source: "Spotify".into(),
      playing: true,
      position_ms: 1_000,
      duration_ms: Some(200_000),
      art_data_url: None,
      captured_at_ms: 10,
    };

    let mut last = LastState::Uninitialized;
    assert!(last.changed(&None));
    last = LastState::NoSession;
    assert!(!last.changed(&None));
    assert!(last.changed(&Some(snapshot.clone())));

    last = LastState::Session(snapshot.clone());
    let mut tick = snapshot.clone();
    tick.position_ms += 100;
    assert!(!last.changed(&Some(tick)));
    assert!(last.changed(&None));
  }
}
