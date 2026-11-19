//! Now playing + transport from Windows SMTC (Phase 3A/3B).
//!
//! One dedicated thread owns the WinRT apartment, the session manager and the
//! album-art cache. Commands arrive over a std channel and are executed on that
//! same thread; the rest of the app only reads the cached snapshot (or listens
//! to `media-state`), so neither the UI thread nor the webview ever blocks on a
//! WinRT `.join()`.

#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Manager;

/// Normal poll cadence for the SMTC thread.
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);
/// Fast cadence for 1.5 s after any transport/seek command.
pub const FAST_POLL_INTERVAL: Duration = Duration::from_millis(150);
pub const FAST_POLL_WINDOW: Duration = Duration::from_millis(1_500);
/// How far the UI's projected position may drift before we resync it.
pub const RESYNC_THRESHOLD_MS: i64 = 1_500;
/// Commands wait this long for the music thread before reporting failure.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

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
  pub playback_rate: f64,
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
      && (self.playback_rate - other.playback_rate).abs() < 1e-6
      && self.duration_ms == other.duration_ms
  }
}

/// Transport command executed on the music thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCommand {
  Toggle,
  Next,
  Prev,
  Seek(i64),
}

struct CommandMessage {
  command: MediaCommand,
  reply: std::sync::mpsc::Sender<bool>,
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
  /// True when the UI needs a fresh payload: track/status changed, the chosen
  /// session switched, or the locally interpolated position drifted too far
  /// from the corrected one (seek elsewhere).
  pub fn changed(
    &self,
    next: &Option<MediaSnapshot>,
    now_ms: i64,
    session_changed: bool,
  ) -> bool {
    if session_changed {
      return true;
    }
    match (self, next) {
      (LastState::Uninitialized, _) => true,
      (LastState::NoSession, None) => false,
      (LastState::NoSession, Some(_)) => true,
      (LastState::Session(_), None) => true,
      (LastState::Session(previous), Some(current)) => {
        !previous.same_state(current) || needs_resync(previous, current, now_ms)
      }
    }
  }
}

/// Rust-owned latest snapshot, shared with the `media_snapshot` command.
#[derive(Default)]
pub struct MediaState {
  cache: Mutex<Cache>,
  ready: Condvar,
  commands: Mutex<Option<std::sync::mpsc::Sender<CommandMessage>>>,
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

  /// Queues a command for the music thread and waits (on a blocking thread)
  /// for its bool reply.
  pub fn send_command(&self, command: MediaCommand, timeout: Duration) -> bool {
    let sender = match self.commands.lock() {
      Ok(guard) => guard.clone(),
      Err(poisoned) => poisoned.into_inner().clone(),
    };
    let Some(sender) = sender else {
      return false;
    };

    let (reply, response) = std::sync::mpsc::channel();
    if sender.send(CommandMessage { command, reply }).is_err() {
      log::warn!("media: command channel closed");
      return false;
    }

    match response.recv_timeout(timeout) {
      Ok(result) => result,
      Err(_) => {
        log::warn!("media: command {command:?} timed out");
        false
      }
    }
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

async fn run_command(state: Arc<MediaState>, command: MediaCommand) -> Result<bool, String> {
  tauri::async_runtime::spawn_blocking(move || state.send_command(command, COMMAND_TIMEOUT))
    .await
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn media_toggle(state: tauri::State<'_, Arc<MediaState>>) -> Result<bool, String> {
  run_command(state.inner().clone(), MediaCommand::Toggle).await
}

#[tauri::command]
pub async fn media_next(state: tauri::State<'_, Arc<MediaState>>) -> Result<bool, String> {
  run_command(state.inner().clone(), MediaCommand::Next).await
}

#[tauri::command]
pub async fn media_prev(state: tauri::State<'_, Arc<MediaState>>) -> Result<bool, String> {
  run_command(state.inner().clone(), MediaCommand::Prev).await
}

#[tauri::command]
pub async fn media_seek(
  state: tauri::State<'_, Arc<MediaState>>,
  position_ms: i64,
) -> Result<bool, String> {
  run_command(
    state.inner().clone(),
    MediaCommand::Seek(position_ms.max(0)),
  )
  .await
}

/// Starts the single SMTC polling thread.
pub fn start(app: tauri::AppHandle) {
  let shared = app.state::<Arc<MediaState>>().inner().clone();
  let (commands, receiver) = std::sync::mpsc::channel();
  match shared.commands.lock() {
    Ok(mut slot) => *slot = Some(commands),
    Err(poisoned) => *poisoned.into_inner() = Some(commands),
  }

  #[cfg(target_os = "windows")]
  imp::start(app, shared, receiver);

  #[cfg(not(target_os = "windows"))]
  {
    let _ = app;
    drop(receiver);
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

/// Unix milliseconds and WinRT ticks (100 ns since 1601-01-01 UTC) from the
/// same instant.
pub fn unix_now() -> (i64, i64) {
  match SystemTime::now().duration_since(UNIX_EPOCH) {
    Ok(duration) => {
      let seconds = duration.as_secs() as i64;
      let nanos = duration.subsec_nanos() as i64;
      (
        seconds * 1_000 + nanos / 1_000_000,
        (seconds + 11_644_473_600) * 10_000_000 + nanos / 100,
      )
    }
    Err(_) => (0, 0),
  }
}

/// Corrects `TimelineProperties::Position()`, which is only a snapshot taken at
/// `LastUpdatedTime()`. While playing, advance it by the elapsed wall time
/// scaled by `playback_rate`. Zero/future timestamps are ignored, and the
/// result is clamped to the track duration when one is known.
pub fn corrected_position_ticks(
  snapshot_ticks: i64,
  last_updated_ticks: i64,
  now_ticks: i64,
  playback_rate: f64,
  playing: bool,
  duration_ticks: Option<i64>,
) -> i64 {
  let adjusted = if playing && last_updated_ticks > 0 && now_ticks >= last_updated_ticks {
    let rate = if playback_rate.is_finite() {
      playback_rate
    } else {
      1.0
    };
    let elapsed = (now_ticks - last_updated_ticks) as f64;
    snapshot_ticks.saturating_add((elapsed * rate).round() as i64)
  } else {
    snapshot_ticks
  };

  match duration_ticks {
    Some(duration) if duration > 0 => adjusted.clamp(0, duration),
    _ => adjusted.max(0),
  }
}

/// Position the UI is currently showing, from the last emitted payload.
pub fn projected_position_ms(snapshot: &MediaSnapshot, now_ms: i64) -> i64 {
  let projected = if snapshot.playing {
    let elapsed = (now_ms - snapshot.captured_at_ms).max(0);
    let delta = (elapsed as f64 * snapshot.playback_rate).round() as i64;
    snapshot.position_ms.saturating_add(delta)
  } else {
    snapshot.position_ms
  };

  match snapshot.duration_ms {
    Some(duration) if duration > 0 => projected.clamp(0, duration),
    _ => projected.max(0),
  }
}

/// True when the real position is far enough from what the UI projects that a
/// fresh `media-state` is needed (a seek that did not change the track/status).
pub fn needs_resync(previous: &MediaSnapshot, current: &MediaSnapshot, now_ms: i64) -> bool {
  (projected_position_ms(previous, now_ms) - current.position_ms).abs() > RESYNC_THRESHOLD_MS
}

#[cfg(target_os = "windows")]
mod imp {
  use super::{
    base64_encode, corrected_position_ticks, source_label, ticks_to_ms, track_duration_ms,
    unix_now, CommandMessage, LastState, MediaCommand, MediaSnapshot, MediaState, FAST_POLL_INTERVAL,
    FAST_POLL_WINDOW, MAX_ART_BYTES, POLL_INTERVAL,
  };
  use std::sync::mpsc::{Receiver, RecvTimeoutError};
  use std::sync::Arc;
  use std::time::Instant;
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

  pub fn start(
    app: tauri::AppHandle,
    shared: Arc<MediaState>,
    commands: Receiver<CommandMessage>,
  ) {
    let spawned = std::thread::Builder::new()
      .name("float-media".into())
      .spawn(move || run(app, shared, commands));

    if let Err(err) = spawned {
      log::error!("media: failed to start poll thread: {err}");
    }
  }

  fn run(
    app: tauri::AppHandle,
    shared: Arc<MediaState>,
    commands: Receiver<CommandMessage>,
  ) {
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
    let mut fast_until: Option<Instant> = None;

    loop {
      let polled = poll_once(
        &mut manager,
        &mut art,
        last_app_id.as_deref(),
        &mut manager_error_logged,
        &mut session_error_logged,
      );

      let now_ms = unix_now().0;
      let session_changed = polled.app_id != last_app_id;
      if session_changed {
        match &polled.app_id {
          Some(app_id) => log::info!("media: session switch -> {app_id}"),
          None => log::info!("media: session switch -> none"),
        }
        last_app_id = polled.app_id.clone();
      }

      if last.changed(&polled.snapshot, now_ms, session_changed) {
        match &polled.snapshot {
          Some(snapshot) => log::info!(
            "media: state -> {} / {} [{}] playing={} rate={} position={}ms duration={:?}ms",
            snapshot.title,
            snapshot.artist,
            snapshot.source,
            snapshot.playing,
            snapshot.playback_rate,
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

      // Wait for the next poll or a command. Commands run on this thread; a
      // command shortens the next poll to 150 ms for 1.5 s.
      let interval = match fast_until {
        Some(until) if Instant::now() < until => FAST_POLL_INTERVAL,
        _ => {
          fast_until = None;
          POLL_INTERVAL
        }
      };
      let deadline = Instant::now() + interval;

      loop {
        let now = Instant::now();
        if now >= deadline {
          break;
        }
        match commands.recv_timeout(deadline - now) {
          Ok(message) => {
            let ok = handle_command(manager.as_ref(), last_app_id.as_deref(), message.command);
            let _ = message.reply.send(ok);
            fast_until = Some(Instant::now() + FAST_POLL_WINDOW);
            break;
          }
          Err(RecvTimeoutError::Timeout) => break,
          Err(RecvTimeoutError::Disconnected) => return,
        }
      }
    }
  }

  fn poll_once(
    manager: &mut Option<Manager>,
    art: &mut ArtCache,
    current_app_id: Option<&str>,
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

    let session = match choose_session(manager, current_app_id) {
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

    let app_id = session_app_id(&session);

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

  fn session_app_id(session: &Session) -> Option<String> {
    session
      .SourceAppUserModelId()
      .map(|id| id.to_string())
      .ok()
  }

  fn is_playing(session: &Session) -> bool {
    session
      .GetPlaybackInfo()
      .and_then(|info| info.PlaybackStatus())
      .map(|status| status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
      .unwrap_or(false)
  }

  /// Stable choice: stay on the shown session while it plays, then prefer the
  /// system's current session, then any playing session, then a paused one.
  fn choose_session(
    manager: &Manager,
    current_app_id: Option<&str>,
  ) -> WinResult<Option<Session>> {
    let sessions = manager.GetSessions()?;
    let count = sessions.Size()?;

    // (a) Keep the session we are showing while it is still playing.
    if let Some(app_id) = current_app_id {
      for index in 0..count {
        let session = sessions.GetAt(index)?;
        if session_app_id(&session).is_some_and(|id| id.eq_ignore_ascii_case(app_id))
          && is_playing(&session)
        {
          return Ok(Some(session));
        }
      }
    }

    let system_current = manager.GetCurrentSession().ok();

    // (b) The system's current session if it is playing.
    if let Some(session) = &system_current {
      if is_playing(session) {
        return Ok(Some(session.clone()));
      }
    }

    // (c) The first playing session on the system.
    for index in 0..count {
      let session = sessions.GetAt(index)?;
      if is_playing(&session) {
        return Ok(Some(session));
      }
    }

    // (d) A paused session, if there is one. No session is a valid idle state.
    Ok(system_current)
  }

  fn find_session(manager: &Manager, app_id: &str) -> WinResult<Option<Session>> {
    let sessions = manager.GetSessions()?;
    let count = sessions.Size()?;
    for index in 0..count {
      let session = sessions.GetAt(index)?;
      if session_app_id(&session).is_some_and(|id| id.eq_ignore_ascii_case(app_id)) {
        return Ok(Some(session));
      }
    }
    Ok(None)
  }

  fn handle_command(
    manager: Option<&Manager>,
    app_id: Option<&str>,
    command: MediaCommand,
  ) -> bool {
    let Some(manager) = manager else {
      log::warn!("media: command {command:?} with no session manager");
      return false;
    };
    let Some(app_id) = app_id else {
      log::warn!("media: command {command:?} with no session");
      return false;
    };

    let session = match find_session(manager, app_id) {
      Ok(Some(session)) => session,
      Ok(None) => {
        log::warn!("media: command {command:?} target {app_id} not found");
        return false;
      }
      Err(err) => {
        log::warn!("media: command {command:?} session lookup failed: {err}");
        return false;
      }
    };

    let result = match command {
      MediaCommand::Toggle => session
        .TryTogglePlayPauseAsync()
        .and_then(|operation| operation.join()),
      MediaCommand::Next => session
        .TrySkipNextAsync()
        .and_then(|operation| operation.join()),
      MediaCommand::Prev => session
        .TrySkipPreviousAsync()
        .and_then(|operation| operation.join()),
      MediaCommand::Seek(position_ms) => session
        .TryChangePlaybackPositionAsync(position_ms.saturating_mul(10_000))
        .and_then(|operation| operation.join()),
    };

    match result {
      Ok(true) => true,
      Ok(false) => {
        log::warn!("media: command {command:?} returned false");
        false
      }
      Err(err) => {
        log::warn!("media: command {command:?} failed: {err}");
        false
      }
    }
  }

  fn read_snapshot(session: &Session, art: &mut ArtCache) -> WinResult<MediaSnapshot> {
    let app_id = session.SourceAppUserModelId()?.to_string();
    let properties = session.TryGetMediaPropertiesAsync()?.join()?;
    let title = properties.Title()?.to_string();
    let artist = properties.Artist()?.to_string();

    let playback = session.GetPlaybackInfo()?;
    let status = playback.PlaybackStatus()?;
    let playing = status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
    let playback_rate = playback
      .PlaybackRate()
      .ok()
      .and_then(|reference| reference.Value().ok())
      .filter(|rate| rate.is_finite())
      .unwrap_or(1.0);

    let timeline = session.GetTimelineProperties()?;
    let start = timeline.StartTime()?.Duration;
    let end = timeline.EndTime()?.Duration;
    let snapshot_position = timeline.Position()?.Duration;
    let last_updated = timeline
      .LastUpdatedTime()
      .map(|time| time.UniversalTime)
      .unwrap_or(0);
    let duration_ticks = end.checked_sub(start).filter(|ticks| *ticks > 0);

    let (captured_at_ms, now_ticks) = unix_now();
    let position = corrected_position_ticks(
      snapshot_position,
      last_updated,
      now_ticks,
      playback_rate,
      playing,
      duration_ticks,
    );

    let art_data_url = art_for(&properties, &title, &artist, art);

    Ok(MediaSnapshot {
      title,
      artist,
      source: source_label(&app_id),
      playing,
      playback_rate,
      position_ms: ticks_to_ms(position),
      duration_ms: track_duration_ms(start, end),
      art_data_url,
      captured_at_ms,
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

  fn base_snapshot() -> MediaSnapshot {
    MediaSnapshot {
      title: "Song".into(),
      artist: "Artist".into(),
      source: "Spotify".into(),
      playing: true,
      playback_rate: 1.0,
      position_ms: 1_000,
      duration_ms: Some(200_000),
      art_data_url: None,
      captured_at_ms: 10,
    }
  }

  #[test]
  fn only_track_state_changes_count_as_new_state() {
    let base = base_snapshot();

    let mut ticked = base.clone();
    ticked.position_ms = 2_000;
    ticked.captured_at_ms = 20;
    assert!(base.same_state(&ticked));

    let mut painted = base.clone();
    painted.art_data_url = Some("data:image/png;base64,AA==".into());
    assert!(base.same_state(&painted));

    let mut slowed = base.clone();
    slowed.playback_rate = 0.5;
    assert!(!base.same_state(&slowed));

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

  /// WinRT ticks: one second is 10_000_000 * 100 ns units.
  const SECOND: i64 = 10_000_000;

  #[test]
  fn paused_position_does_not_advance() {
    let last = 100 * SECOND;
    assert_eq!(
      corrected_position_ticks(
        5 * SECOND,
        last,
        last + 15 * SECOND,
        2.0,
        false,
        Some(200 * SECOND),
      ),
      5 * SECOND
    );
  }

  #[test]
  fn playing_advances_by_elapsed_time_since_last_update() {
    let last = 100 * SECOND;
    assert_eq!(
      corrected_position_ticks(
        5 * SECOND,
        last,
        last + 15 * SECOND,
        1.0,
        true,
        Some(200 * SECOND),
      ),
      20 * SECOND
    );
  }

  #[test]
  fn playback_rate_scales_the_elapsed_time() {
    let last = 100 * SECOND;
    assert_eq!(
      corrected_position_ticks(0, last, last + 10 * SECOND, 2.0, true, None),
      20 * SECOND
    );
  }

  #[test]
  fn zero_or_future_last_updated_time_is_ignored() {
    let now = 100 * SECOND;
    assert_eq!(
      corrected_position_ticks(5 * SECOND, 0, now, 2.0, true, None),
      5 * SECOND
    );
    assert_eq!(
      corrected_position_ticks(5 * SECOND, now + SECOND, now, 2.0, true, None),
      5 * SECOND
    );
  }

  #[test]
  fn position_is_clamped_to_the_duration() {
    let last = 100 * SECOND;
    assert_eq!(
      corrected_position_ticks(
        15 * SECOND,
        last,
        last + 15 * SECOND,
        5.0,
        true,
        Some(20 * SECOND),
      ),
      20 * SECOND
    );
  }

  #[test]
  fn drift_only_triggers_above_the_resync_threshold() {
    let base = base_snapshot();
    let mut moved = base.clone();
    moved.position_ms += 1_000;
    assert!(!needs_resync(&base, &moved, base.captured_at_ms));

    moved.position_ms += 1_000;
    assert!(needs_resync(&base, &moved, base.captured_at_ms));
  }

  #[test]
  fn last_state_emits_on_track_session_or_seek_changes() {
    let snapshot = base_snapshot();
    let now = snapshot.captured_at_ms;

    let mut last = LastState::Uninitialized;
    assert!(last.changed(&None, now, false));
    last = LastState::NoSession;
    assert!(!last.changed(&None, now, false));
    assert!(last.changed(&Some(snapshot.clone()), now, false));

    last = LastState::Session(snapshot.clone());
    let mut tick = snapshot.clone();
    tick.position_ms += 100;
    assert!(!last.changed(&Some(tick), now, false));

    assert!(last.changed(&Some(snapshot.clone()), now, true));

    let mut seeked = snapshot.clone();
    seeked.position_ms += RESYNC_THRESHOLD_MS + 1;
    assert!(last.changed(&Some(seeked), now, false));

    assert!(last.changed(&None, now, false));
  }
}
