//! Notes helpers and Tauri commands.

use std::path::PathBuf;
use tauri::Manager;

/// A parsed note: title is the first non-empty line (with leading "#"
/// characters and spaces stripped), body is everything after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
  pub title: String,
  pub body: String,
}

/// Normalise CRLF to LF and strip a UTF-8 BOM.
fn normalise_text(text: &str) -> String {
  let s = text.strip_prefix("\u{FEFF}").unwrap_or(text);
  s.replace("\r\n", "\n")
}

/// Parse a note from raw text.
/// - title = the first non-empty line, with leading "#" characters and
///   spaces removed.
/// - body = everything after that line, with ONE leading blank line removed.
/// - If there is no non-empty line, title and body are both empty.
pub fn parse_note(text: &str) -> Note {
  let text = normalise_text(text);
  let mut title = String::new();
  let mut body_lines: Vec<String> = Vec::new();
  let mut found_title = false;
  let mut body_started = false;
  let mut skipped_one_blank = false;

  for line in text.lines() {
    let trimmed = line.trim();
    if !found_title && !trimmed.is_empty() {
      // This is the title line: strip leading "#" characters and spaces
      let mut stripped = trimmed.to_string();
      while stripped.starts_with('#') || stripped.starts_with(' ') {
        if stripped.starts_with('#') {
          stripped = stripped[1..].to_string();
        } else if stripped.starts_with(' ') {
          stripped = stripped[1..].to_string();
        }
      }
      title = stripped;
      found_title = true;
      body_started = true;
    } else if found_title {
      // We're in the body
      if !body_started {
        // First body line – may be a blank line after the title;
        // skip exactly one blank line
        if trimmed.is_empty() && !skipped_one_blank {
          skipped_one_blank = true;
          continue;
        }
        body_started = true;
        if !trimmed.is_empty() {
          body_lines.push(trimmed.to_string());
        }
      } else {
        // Subsequent body line
        if !trimmed.is_empty() {
          body_lines.push(trimmed.to_string());
        }
      }
    }
    // If we haven't found a title yet and the line is empty, just skip it.
  }

  // If we never found a non-empty line, title and body are both empty.
  if !found_title {
    Note {
      title: String::new(),
      body: String::new(),
    }
  } else {
    let body = if body_lines.is_empty() {
      String::new()
    } else {
      body_lines.join("\n")
    };
    Note {
      title,
      body,
    }
  }
}

/// Serialize a note back to string.
/// - Non-empty title: "# {title}\n\n{body}" (just "# {title}\n" if body is empty).
/// - Empty title: the body as-is.
pub fn serialize_note(title: &str, body: &str) -> String {
  if title.is_empty() {
    // Empty title: body as is.
    body.to_string()
  } else {
    if body.is_empty() {
      format!("# {}\n", title)
    } else {
      format!("# {}\n\n{}", title, body)
    }
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

/// Create a file name from the current local time and a slug.
/// Format: "YYYY-MM-DD HH-mm-ss {slug}.md"
#[tauri::command]
pub fn file_name(slug: &str, year: u32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> String {
  format!("{:04}-{:02}-{:02} {:02}-{:02}-{:02} {}.md", year, month, day, hour, minute, second, slug)
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
  let documents = app.path().document_dir().map_err(|error| error.to_string())?;
  Ok(documents.join("Float Notes"))
}

/// Note ids are file names: reject separators, traversal, drive prefixes and
/// characters Windows cannot store.
pub fn valid_note_id(id: &str) -> bool {
  !id.is_empty()
    && id == sanitise_id(id)
    && !id.contains(':')
    && !id.chars().any(|c| c.is_control() || matches!(c, '*' | '?' | '"' | '<' | '>' | '|'))
}

/// List notes (limited to default 5, newest first).
/// Returns {id, title, modifiedMs}.
#[tauri::command]
pub fn notes_list(_limit: Option<usize>) -> Result<Vec<(String, String, u64)>, String> {
  Ok(vec![])
}

/// Get a note by id.
/// Returns {id, title, body, modifiedMs}.
#[tauri::command]
pub fn notes_get(_id: String) -> Result<(String, String, String, u64), String> {
  Ok((String::new(), String::new(), String::new(), 0))
}

/// Save a note (create or update).
#[tauri::command]
pub fn notes_save(_id: Option<String>, _title: String, _body: String) -> Result<(Option<String>, u64), String> {
  Ok((None, 0))
}

/// Create a new note with given title and body.
/// If title and body are both empty, returns null id.
#[tauri::command]
pub fn notes_create(_title: String, _body: String) -> Result<(Option<String>, u64), String> {
  Ok((None, 0))
}

/// Delete a note by id: move to the Recycle Bin with trash::delete.
#[tauri::command]
pub fn notes_delete(_id: String) -> Result<(), String> {
  Ok(())
}

/// Open the notes folder: create if needed, then run explorer.exe with the path.
#[tauri::command]
pub fn notes_open_folder(app: tauri::AppHandle) -> Result<(), String> {
  let settings = crate::settings::load_or_default(&app);
  let dir = resolve_notes_dir(&app, settings.notes_folder)?;
  std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;

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
  fn body_starting_with_blank_line() {
    let text = "# My Note\n\n\nThis is the body.\nSecond line.";
    let note = parse_note(text);
    assert_eq!(note.title, "My Note");
    assert_eq!(note.body, "This is the body.\nSecond line.");
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
    assert_eq!(note.body, "Body text.");
  }

  #[test]
  fn serialize_empty_title_with_body() {
    let serialized = serialize_note("", "Just body");
    assert_eq!(serialized, "Just body");
  }

  #[test]
  fn serialize_nonempty_title_empty_body() {
    let serialized = serialize_note("Title", "");
    assert_eq!(serialized, "# Title\n");
  }

  #[test]
  fn serialize_nonempty_title_nonempty_body() {
    let serialized = serialize_note("Title", "Body text");
    assert_eq!(serialized, "# Title\n\nBody text");
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
  fn reveal_rejects_unsafe_ids() {
    assert!(valid_note_id("2026-11-24 10-00-00 Note.md"));
    assert!(!valid_note_id(""));
    assert!(!valid_note_id("../secret.md"));
    assert!(!valid_note_id("C:\\secret.md"));
    assert!(!valid_note_id("sub/note.md"));
  }
}