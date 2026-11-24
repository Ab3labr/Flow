//! Notes helpers and Tauri commands.

use serde::Serialize;
use std::{
  fs::{self, File},
  io::Read,
  path::{Path, PathBuf},
  sync::atomic::{AtomicU64, Ordering},
  time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager};

const MAX_NOTE_BYTES: u64 = 2 * 1024 * 1024;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A parsed note: title is the first non-empty line (with leading "#"
/// characters and spaces stripped), body is everything after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
  pub title: String,
  pub body: String,
}

/// List row returned to the frontend.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
  pub id: String,
  pub title: String,
  pub modified_ms: u64,
}

/// One note returned by `notes_get`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteFile {
  pub id: String,
  pub title: String,
  pub body: String,
  pub modified_ms: u64,
}

/// Result of `notes_save` / `notes_create`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
  pub id: Option<String>,
  pub modified_ms: u64,
}

/// Normalise CRLF to LF and strip a UTF-8 BOM.
fn normalise_text(text: &str) -> String {
  let s = text.strip_prefix("\u{FEFF}").unwrap_or(text);
  s.replace("\r\n", "\n")
}

/// Strip leading `#` characters and spaces from a title line.
fn strip_title_prefix(line: &str) -> String {
  let mut stripped = line.trim().to_string();
  while stripped.starts_with('#') || stripped.starts_with(' ') {
    if stripped.starts_with('#') {
      stripped = stripped[1..].to_string();
    } else {
      stripped = stripped[1..].to_string();
    }
  }
  stripped
}

/// Parse a note from raw text.
/// - title = the first non-empty line, with leading "#" characters and
///   spaces removed. A line that is only `#` is an explicit empty title.
/// - body = everything after that line, with ONE blank separator line removed.
///   All other blank lines, including trailing ones, are preserved exactly.
/// - If there is no non-empty line, title and body are both empty.
pub fn parse_note(text: &str) -> Note {
  let text = normalise_text(text);
  let lines: Vec<&str> = text.split('\n').collect();

  let mut title = String::new();
  let mut title_index = None;
  for (index, line) in lines.iter().enumerate() {
    if line.trim().is_empty() {
      continue;
    }
    let stripped = strip_title_prefix(line);
    // `#` (or `###`) on its own is the empty-title marker written by
    // serialize_note. A body-only file from an older version still parses
    // its first line as the title.
    if stripped.is_empty() && line.trim().starts_with('#') {
      title_index = Some(index);
      break;
    }
    if !stripped.is_empty() {
      title = stripped;
      title_index = Some(index);
      break;
    }
  }

  let Some(index) = title_index else {
    return Note {
      title: String::new(),
      body: String::new(),
    };
  };

  let mut body_lines = &lines[index + 1..];
  // serialize_note writes one blank line between the heading and the body.
  // Skip exactly that line; a second blank line belongs to the body.
  if body_lines.first().is_some_and(|line| line.trim().is_empty()) {
    body_lines = &body_lines[1..];
  }

  Note {
    title,
    body: body_lines.join("\n"),
  }
}

/// Serialize a note back to string. An empty title gets an explicit `#`
/// marker so a body beginning with text cannot be mistaken for a title.
pub fn serialize_note(title: &str, body: &str) -> String {
  if title.is_empty() {
    if body.is_empty() {
      String::new()
    } else {
      format!("#\n\n{body}")
    }
  } else if body.is_empty() {
    format!("# {title}\n")
  } else {
    format!("# {title}\n\n{body}")
  }
}

/// Remove / \ : * ? " < > | and control characters from a string.
/// Then collapse whitespace, trim spaces and dots, and cap at 50 characters.
/// Empty result becomes "Untitled".
pub fn slugify(title: &str) -> String {
  let forbidden = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
  let mut result = title.to_string();
  result.retain(|c| !forbidden.contains(&c) && !c.is_control());
  let mut collapsed = String::new();
  let mut prev_was_space = false;
  for c in result.chars() {
    if c.is_whitespace() {
      if !prev_was_space {
        collapsed.push(' ');
        prev_was_space = true;
      }
    } else {
      collapsed.push(c);
      prev_was_space = false;
    }
  }
  let trimmed = collapsed.trim_matches(' ').trim_matches('.');
  let capped = trimmed.chars().take(50).collect::<String>();
  if capped.trim().is_empty() {
    "Untitled".to_string()
  } else {
    capped
  }
}

/// File-name slugs must never contain `..`, which `valid_note_id` rejects.
fn safe_slug(title: &str) -> String {
  let mut slug = slugify(title);
  while slug.contains("..") {
    slug = slug.replace("..", ".");
  }
  slug
}

/// Create a file name from the current local time and a slug.
/// Format: "YYYY-MM-DD HH-mm-ss {slug}.md"
#[tauri::command]
pub fn file_name(
  slug: &str,
  year: u32,
  month: u32,
  day: u32,
  hour: u32,
  minute: u32,
  second: u32,
) -> String {
  format!(
    "{year:04}-{month:02}-{day:02} {hour:02}-{minute:02}-{second:02} {slug}.md"
  )
}

/// Remove non-path-separator characters from an id string.
/// Ids containing ".." or path separators are rejected for security.
fn sanitise_id(id: &str) -> String {
  id.replace("\\", "")
    .replace("/", "")
    .replace("..", "")
}

/// Return the notes folder path based on settings.
pub fn notes_dir(settings_notes_folder: Option<String>) -> String {
  if let Some(folder) = settings_notes_folder {
    folder
  } else {
    "Documents/Float Notes".to_string()
  }
}

/// Absolute notes folder: an explicit override, or Documents\Float Notes.
pub fn resolve_notes_dir(
  app: &tauri::AppHandle,
  settings_notes_folder: Option<String>,
) -> Result<PathBuf, String> {
  if let Some(folder) = settings_notes_folder {
    return Ok(PathBuf::from(folder));
  }
  let documents = app
    .path()
    .document_dir()
    .map_err(|error| error.to_string())?;
  Ok(documents.join("Float Notes"))
}

/// Note ids are file names: reject separators, traversal, drive prefixes and
/// characters Windows cannot store.
pub fn valid_note_id(id: &str) -> bool {
  !id.is_empty()
    && id == sanitise_id(id)
    && !id.contains(':')
    && !id
      .chars()
      .any(|c| c.is_control() || matches!(c, '*' | '?' | '"' | '<' | '>' | '|'))
}

fn system_millis(time: SystemTime) -> u64 {
  time
    .duration_since(UNIX_EPOCH)
    .map(|duration| duration.as_millis() as u64)
    .unwrap_or(0)
}

fn note_path(dir: &Path, id: &str) -> Result<PathBuf, String> {
  if !valid_note_id(id) {
    return Err("Invalid note id".to_string());
  }
  Ok(dir.join(id))
}

fn modified_ms(path: &Path) -> u64 {
  fs::metadata(path)
    .and_then(|metadata| metadata.modified())
    .map(system_millis)
    .unwrap_or(0)
}

fn read_note_file(dir: &Path, id: &str) -> Result<NoteFile, String> {
  let path = note_path(dir, id)?;
  let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
  if metadata.len() > MAX_NOTE_BYTES {
    return Err("Note too large to open here".to_string());
  }
  let bytes = fs::read(&path).map_err(|error| error.to_string())?;
  let text = String::from_utf8(bytes).map_err(|error| error.to_string())?;
  let parsed = parse_note(&text);
  Ok(NoteFile {
    id: id.to_string(),
    title: parsed.title,
    body: parsed.body,
    modified_ms: metadata
      .modified()
      .map(system_millis)
      .unwrap_or_else(|_| modified_ms(&path)),
  })
}

fn read_title_prefix(path: &Path) -> String {
  let mut file = match File::open(path) {
    Ok(file) => file,
    Err(_) => return String::new(),
  };
  let mut buffer = [0u8; 4096];
  let read = file.read(&mut buffer).unwrap_or(0);
  parse_note(&String::from_utf8_lossy(&buffer[..read])).title
}

fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
  let parent = path
    .parent()
    .ok_or_else(|| "Missing notes directory".to_string())?;
  fs::create_dir_all(parent).map_err(|error| error.to_string())?;
  let name = path
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| "Invalid note file name".to_string())?;
  let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
  let temp = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), sequence));
  fs::write(&temp, content).map_err(|error| error.to_string())?;
  if let Err(error) = fs::rename(&temp, path) {
    let _ = fs::remove_file(&temp);
    return Err(error.to_string());
  }
  Ok(())
}

/// Convert days since the Unix epoch to a civil date (UTC fallback).
#[cfg(not(windows))]
fn civil_from_days(days: i64) -> (i64, i64, i64) {
  let z = days + 719_468;
  let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
  let doe = z - era * 146_097;
  let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
  let year = yoe + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let day = doy - (153 * mp + 2) / 5 + 1;
  let month = mp + if mp < 10 { 3 } else { -9 };
  (year + if month <= 2 { 1 } else { 0 }, month, day)
}

#[cfg(not(windows))]
fn utc_stamp(millis: u64) -> String {
  let seconds = (millis / 1000) as i64;
  let days = seconds.div_euclid(86_400);
  let within_day = seconds.rem_euclid(86_400);
  let (year, month, day) = civil_from_days(days);
  format!(
    "{year:04}-{month:02}-{day:02} {:02}-{:02}-{:02}",
    within_day / 3600,
    (within_day % 3600) / 60,
    within_day % 60
  )
}

#[cfg(windows)]
fn note_stamp() -> String {
  unsafe {
    let time = windows::Win32::System::SystemInformation::GetLocalTime();
    format!(
      "{:04}-{:02}-{:02} {:02}-{:02}-{:02}",
      time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
    )
  }
}

#[cfg(not(windows))]
fn note_stamp() -> String {
  utc_stamp(system_millis(SystemTime::now()))
}

fn unique_note_id(dir: &Path, title: &str, stamp: &str) -> String {
  let stem = format!("{stamp} {}", safe_slug(title));
  let mut candidate = format!("{stem}.md");
  let mut suffix = 2;
  while dir.join(&candidate).exists() {
    candidate = format!("{stem} {suffix}.md");
    suffix += 1;
  }
  candidate
}

fn save_note_in_dir(
  dir: &Path,
  id: Option<&str>,
  title: &str,
  body: &str,
  stamp: &str,
) -> Result<SaveResult, String> {
  fs::create_dir_all(dir).map_err(|error| error.to_string())?;

  if title.trim().is_empty() && body.trim().is_empty() {
    let existing = id
      .filter(|id| valid_note_id(id) && dir.join(id).is_file())
      .map(str::to_string);
    return Ok(SaveResult {
      id: existing,
      modified_ms: system_millis(SystemTime::now()),
    });
  }

  let content = serialize_note(title, body);
  let mut target_id = None;

  if let Some(id) = id {
    if valid_note_id(id) {
      let old_path = dir.join(id);
      let mut final_path = old_path.clone();
      // A note first saved with an empty title gets a real file name as soon
      // as a title arrives; the markdown content is unchanged.
      let file_name = old_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
      if file_name.ends_with(" Untitled.md") && !title.trim().is_empty() {
        let renamed = format!(
          "{} {}.md",
          file_name.trim_end_matches(" Untitled.md"),
          safe_slug(title)
        );
        let renamed_path = dir.join(&renamed);
        if !renamed_path.exists() && fs::rename(&old_path, &renamed_path).is_ok() {
          final_path = renamed_path;
        }
      }
      if final_path.is_file() {
        atomic_write(&final_path, &content)?;
        target_id = final_path
          .file_name()
          .and_then(|name| name.to_str())
          .map(str::to_string);
      }
    }
  }

  let target_id = match target_id {
    Some(id) => id,
    None => {
      let created = unique_note_id(dir, title, stamp);
      atomic_write(&dir.join(&created), &content)?;
      created
    }
  };

  let modified_ms = modified_ms(&dir.join(&target_id));
  Ok(SaveResult {
    id: Some(target_id),
    modified_ms,
  })
}

fn list_notes_in_dir(dir: &Path, limit: Option<usize>) -> Result<Vec<NoteMeta>, String> {
  fs::create_dir_all(dir).map_err(|error| error.to_string())?;
  let mut rows = Vec::new();
  for entry in fs::read_dir(dir).map_err(|error| error.to_string())? {
    let entry = entry.map_err(|error| error.to_string())?;
    let path = entry.path();
    if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
      continue;
    }
    let id = path
      .file_name()
      .and_then(|name| name.to_str())
      .unwrap_or("")
      .to_string();
    if !valid_note_id(&id) {
      continue;
    }
    rows.push(NoteMeta {
      id,
      title: read_title_prefix(&path),
      modified_ms: modified_ms(&path),
    });
  }
  rows.sort_by(|a, b| {
    b.modified_ms
      .cmp(&a.modified_ms)
      .then_with(|| b.id.cmp(&a.id))
  });
  if let Some(limit) = limit {
    rows.truncate(limit);
  }
  Ok(rows)
}

fn delete_note_in_dir(dir: &Path, id: &str) -> Result<(), String> {
  let path = note_path(dir, id)?;
  if !path.is_file() {
    return Ok(());
  }
  #[cfg(windows)]
  {
    trash::delete(&path).map_err(|error| error.to_string())
  }
  #[cfg(not(windows))]
  {
    fs::remove_file(&path).map_err(|error| error.to_string())
  }
}

/// List notes (default 5, newest first).
#[tauri::command]
pub fn notes_list(
  app: tauri::AppHandle,
  limit: Option<usize>,
) -> Result<Vec<NoteMeta>, String> {
  let settings = crate::settings::load_or_default(&app);
  let dir = resolve_notes_dir(&app, settings.notes_folder)?;
  list_notes_in_dir(&dir, Some(limit.unwrap_or(5)))
}

/// Get a note by id.
#[tauri::command]
pub fn notes_get(app: tauri::AppHandle, id: String) -> Result<NoteFile, String> {
  let settings = crate::settings::load_or_default(&app);
  let dir = resolve_notes_dir(&app, settings.notes_folder)?;
  read_note_file(&dir, &id)
}

/// Save a note (create or update), with an atomic temp-file rename.
#[tauri::command]
pub fn notes_save(
  app: tauri::AppHandle,
  id: Option<String>,
  title: String,
  body: String,
) -> Result<SaveResult, String> {
  let settings = crate::settings::load_or_default(&app);
  let dir = resolve_notes_dir(&app, settings.notes_folder)?;
  let result = save_note_in_dir(&dir, id.as_deref(), &title, &body, &note_stamp())?;
  let _ = app.emit("notes-changed", ());
  Ok(result)
}

/// Create a note (kept for parity with the mock service).
#[tauri::command]
pub fn notes_create(
  app: tauri::AppHandle,
  title: String,
  body: String,
) -> Result<SaveResult, String> {
  let settings = crate::settings::load_or_default(&app);
  let dir = resolve_notes_dir(&app, settings.notes_folder)?;
  let result = save_note_in_dir(&dir, None, &title, &body, &note_stamp())?;
  let _ = app.emit("notes-changed", ());
  Ok(result)
}

/// Delete a note by id: move to the Recycle Bin with trash::delete.
#[tauri::command]
pub fn notes_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
  let settings = crate::settings::load_or_default(&app);
  let dir = resolve_notes_dir(&app, settings.notes_folder)?;
  delete_note_in_dir(&dir, &id)?;
  let _ = app.emit("notes-changed", ());
  Ok(())
}

/// Open the notes folder: create if needed, then run explorer.exe with the path.
#[tauri::command]
pub fn notes_open_folder(app: tauri::AppHandle) -> Result<(), String> {
  let settings = crate::settings::load_or_default(&app);
  let dir = resolve_notes_dir(&app, settings.notes_folder)?;
  fs::create_dir_all(&dir).map_err(|error| error.to_string())?;

  #[cfg(windows)]
  {
    std::process::Command::new("explorer.exe")
      .arg(&dir)
      .spawn()
      .map_err(|error| error.to_string())?;
    Ok(())
  }
  #[cfg(not(windows))]
  {
    let _ = dir;
    Ok(())
  }
}

/// Reveal one note in Explorer with the file selected.
#[tauri::command]
pub fn notes_reveal(app: tauri::AppHandle, id: String) -> Result<(), String> {
  if !valid_note_id(&id) {
    return Err("Invalid note id".to_string());
  }

  let settings = crate::settings::load_or_default(&app);
  let dir = resolve_notes_dir(&app, settings.notes_folder)?;
  let path = dir.join(&id);

  #[cfg(windows)]
  {
    std::process::Command::new("explorer.exe")
      .arg(format!("/select,{}", path.display()))
      .spawn()
      .map_err(|error| error.to_string())?;
    Ok(())
  }
  #[cfg(not(windows))]
  {
    let _ = path;
    Err("Not supported on this platform".to_string())
  }
}

/// Get the notes folder path.
#[tauri::command]
pub fn notes_folder_path(app: tauri::AppHandle) -> Result<String, String> {
  let settings = crate::settings::load_or_default(&app);
  let dir = resolve_notes_dir(&app, settings.notes_folder)?;
  Ok(dir.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("float-notes-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create test notes dir");
    dir
  }

  #[test]
  fn round_trip_no_newline_title() {
    let title = "My Meeting";
    let body = "Discuss the project timeline";
    let serialized = serialize_note(title, body);
    let parsed = parse_note(&serialized);
    assert_eq!(parsed.title, title);
    assert_eq!(parsed.body, body);
  }

  #[test]
  fn round_trip_empty_body() {
    let title = "My Meeting";
    let body = "";
    let serialized = serialize_note(title, body);
    let parsed = parse_note(&serialized);
    assert_eq!(parsed.title, title);
    assert_eq!(parsed.body, "");
  }

  #[test]
  fn empty_parts() {
    let note = parse_note("");
    assert_eq!(note.title, "");
    assert_eq!(note.body, "");

    let serialized = serialize_note("", "");
    assert_eq!(serialized, "");
  }

  #[test]
  fn round_trip_empty_title_with_body() {
    let body = "First line\n\nThird line\n";
    let serialized = serialize_note("", body);
    let parsed = parse_note(&serialized);
    assert_eq!(parsed.title, "");
    assert_eq!(parsed.body, body);
  }

  #[test]
  fn round_trip_preserves_blank_lines() {
    let title = "Title";
    let body = "One\n\nTwo\n\n\nThree\n";
    let parsed = parse_note(&serialize_note(title, body));
    assert_eq!(parsed.title, title);
    assert_eq!(parsed.body, body);
  }

  #[test]
  fn body_starting_with_blank_line_is_preserved() {
    let text = "# My Note\n\n\nThis is the body.\nSecond line.";
    let note = parse_note(text);
    assert_eq!(note.title, "My Note");
    assert_eq!(note.body, "\nThis is the body.\nSecond line.");
  }

  #[test]
  fn hash_inside_title() {
    let text = "# Title with # inside\nBody text.";
    let note = parse_note(text);
    assert_eq!(note.title, "Title with # inside");
    assert_eq!(note.body, "Body text.");
  }

  #[test]
  fn unicode_title() {
    let text = "Réunion\nBody text.";
    let note = parse_note(text);
    assert_eq!(note.title, "Réunion");
    assert_eq!(note.body, "Body text.");
  }

  #[test]
  fn crlf_input() {
    let text = "# My Note\r\nBody text.\r\n";
    let note = parse_note(text);
    assert_eq!(note.title, "My Note");
    assert_eq!(note.body, "Body text.\n");
  }

  #[test]
  fn bom_is_stripped() {
    let text = "\u{FEFF}# Title\nBody";
    let note = parse_note(text);
    assert_eq!(note.title, "Title");
    assert_eq!(note.body, "Body");
  }

  #[test]
  fn serialize_empty_title_with_body_uses_marker() {
    assert_eq!(serialize_note("", "Just body"), "#\n\nJust body");
  }

  #[test]
  fn serialize_nonempty_title_empty_body() {
    assert_eq!(serialize_note("Title", ""), "# Title\n");
  }

  #[test]
  fn serialize_nonempty_title_nonempty_body() {
    assert_eq!(serialize_note("Title", "Body text"), "# Title\n\nBody text");
  }

  #[test]
  fn slugify_basic() {
    assert_eq!(slugify("Hello World"), "Hello World");
  }

  #[test]
  fn slugify_remove_forbidden() {
    assert_eq!(slugify("hello:world?"), "helloworld");
  }

  #[test]
  fn slugify_collapse_whitespace() {
    assert_eq!(slugify("  hello   world  "), "hello world");
  }

  #[test]
  fn slugify_cap_50() {
    let long = "A".repeat(60);
    let s = slugify(&long);
    assert!(s.len() <= 50);
  }

  #[test]
  fn slugify_empty_becomes_untitled() {
    assert_eq!(slugify(""), "Untitled");
  }

  #[test]
  fn slugify_trailing_dots_trimmed() {
    assert_eq!(slugify("..hello.."), "hello");
  }

  #[test]
  fn safe_slug_removes_traversal_dots() {
    assert_eq!(safe_slug("a..b"), "a.b");
  }

  #[test]
  fn reveal_rejects_unsafe_ids() {
    assert!(valid_note_id("2026-11-24 10-00-00 Note.md"));
    assert!(!valid_note_id(""));
    assert!(!valid_note_id("../secret.md"));
    assert!(!valid_note_id("C:\\secret.md"));
    assert!(!valid_note_id("sub/note.md"));
  }

  #[test]
  fn save_get_and_list_preserve_blank_lines() {
    let dir = test_dir("roundtrip");
    let saved = save_note_in_dir(
      &dir,
      None,
      "Meeting",
      "First\n\nSecond\n",
      "2026-11-24 10-00-00",
    )
    .expect("save note");
    let id = saved.id.expect("saved id");
    assert!(id.ends_with(" Meeting.md"));

    let note = read_note_file(&dir, &id).expect("read note");
    assert_eq!(note.title, "Meeting");
    assert_eq!(note.body, "First\n\nSecond\n");

    let rows = list_notes_in_dir(&dir, Some(5)).expect("list notes");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, id);
    assert_eq!(rows[0].title, "Meeting");

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn saving_existing_note_keeps_its_id_and_updates_content() {
    let dir = test_dir("update");
    let first = save_note_in_dir(
      &dir,
      None,
      "Title",
      "Old body",
      "2026-11-24 10-00-00",
    )
    .expect("save note");
    let id = first.id.expect("saved id").clone();

    let second = save_note_in_dir(
      &dir,
      Some(&id),
      "Title",
      "New body\n\nWith a blank line",
      "2026-11-24 11-00-00",
    )
    .expect("update note");
    assert_eq!(second.id.as_deref(), Some(id.as_str()));
    assert_eq!(read_note_file(&dir, &id).expect("read note").body, "New body\n\nWith a blank line");

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn empty_title_note_gets_a_real_name_when_titled() {
    let dir = test_dir("rename");
    let first = save_note_in_dir(&dir, None, "", "A thought", "2026-11-24 10-00-00")
      .expect("save note");
    let id = first.id.expect("saved id");
    assert!(id.ends_with(" Untitled.md"));
    assert_eq!(read_note_file(&dir, &id).expect("read note").title, "");

    let renamed = save_note_in_dir(
      &dir,
      Some(&id),
      "Real title",
      "A thought",
      "2026-11-24 11-00-00",
    )
    .expect("title note");
    let new_id = renamed.id.expect("renamed id");
    assert!(new_id.ends_with(" Real title.md"));
    assert!(!dir.join(&id).exists());
    assert_eq!(read_note_file(&dir, &new_id).expect("read note").title, "Real title");

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn list_sorts_newest_first_and_honours_limit() {
    let dir = test_dir("list");
    save_note_in_dir(&dir, None, "Old", "old", "2026-11-23 10-00-00").expect("save old");
    save_note_in_dir(&dir, None, "New", "new", "2026-11-24 10-00-00").expect("save new");

    let rows = list_notes_in_dir(&dir, Some(1)).expect("list notes");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "New");

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn oversized_note_is_rejected_on_read() {
    let dir = test_dir("large");
    let id = "2026-11-24 10-00-00 Big.md";
    fs::write(dir.join(id), vec![b'a'; (MAX_NOTE_BYTES + 1) as usize]).expect("write big note");
    assert!(read_note_file(&dir, id).is_err());

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn blank_note_does_not_create_a_file() {
    let dir = test_dir("blank");
    let saved = save_note_in_dir(&dir, None, "  ", "\n  \n", "2026-11-24 10-00-00")
      .expect("save blank");
    assert_eq!(saved.id, None);
    assert_eq!(list_notes_in_dir(&dir, None).expect("list notes").len(), 0);

    let _ = fs::remove_dir_all(&dir);
  }
}
