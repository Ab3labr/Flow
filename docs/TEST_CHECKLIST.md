# TEST CHECKLIST — Float Notes

## Manual Notes Tests (Phase 5B+C)

### Draft Behavior
- [ ] "New note" creates an in-memory draft with no file path
- [ ] First save with non-empty content creates the file (Rust returns the id)
- [ ] Later saves use the existing id
- [ ] Never create files when both title and body are empty
- [ ] Two quick edits produce one queued save (autosave debounced at 600 ms)
- [ ] Empty draft never saves; setSaved stays true

### Autosave Behavior
- [ ] 600 ms debounce after last keystroke
- [ ] Pending save flushed IMMEDIATELY (not debounced) when:
  - [ ] User switches tab
  - [ ] Panel closes or collapses (Esc, outside click, hover-close, double-Alt)
  - [ ] Window is hidden or loses focus
  - [ ] User presses New note or opens another note
  - [ ] "flush-request" from Rust (then emit "flush-done" from frontend)
- [ ] Never run two saves for the same note at once
- [ ] If keystroke arrives during save, queue one more save with latest text
- [ ] "Saving..." indicator shown if save takes longer than 400 ms
- [ ] "Couldn't save" in red (#ff453a) on failure; text kept in editor, retries on next change
- [ ] Footer checkmark after successful save

### Recent List Popover
- [ ] Top 5 from list(5) displayed with title ("Untitled note" if empty) and time ago
- [ ] Clicking opens that note (flushes current one first)
- [ ] Hover shows trash icon that calls remove() (Recycle Bin)
- [ ] If the open note is deleted, switch to a new draft
- [ ] Note deleted outside Float: switch to draft keeping current text

### Notes Folder Path
- [ ] "Notes folder" row shows real path, shortened with middle ellipsis if long
- [ ] Full path in tooltip
- [ ] "Open folder" button calls notes_open_folder
- [ ] No folder picker yet (Phase 8)
- [ ] To change folder: edit notesFolder value in settings.json and restart
- [ ] Default: Documents\\Float Notes (or %APPDATA%\\Float\\settings.json location)

### Edge Cases
- [ ] Folder deleted while running: recreate on next save (no crash)
- [ ] Read-only or locked file: show "Couldn't save" in red; keep text in editor
- [ ] Very long titles: slug capped at 50 chars; trailing dots trimmed
- [ ] Note over 2 MB: listed in recent list, but opening shows "Too large to open here" with Open folder button
- [ ] CRLF files: normalized LF on read; title extraction works correctly
- [ ] UTF-8 BOM files: stripped on read; title extraction works correctly

### Tauri Commands
- [ ] notes_list returns recent notes sorted by mtime newest first
- [ ] notes_get returns note data; rejects files over 2 MB
- [ ] notes_save creates/updates note atomically (temp + rename)
- [ ] notes_delete moves to Recycle Bin
- [ ] notes_open_folder opens explorer.exe with the notes folder path
- [ ] notes_folder_path returns the correct path
- [ ] notes_create creates in-memory draft

### Rust Backend
- [ ] Folder watcher with notify crate: non-recursive, 150 ms debounce, 500 ms own-write ignore
- [ ] Emit "notes-changed" event
- [ ] Flush on quit: tray "Quit" emits "flush-request" waits up to 800 ms for "flush-done", then exits
- [ ] Trash crate for deletes (cross-platform Recycle Bin)

### Build and Test
- [ ] `cargo check` — clean
- [ ] `cargo test` — all tests passing
- [ ] `pnpm build` — front-end build succeeds
- [ ] `pnpm test` — front-end tests pass