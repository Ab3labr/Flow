with open('docs/PROGRESS.md', 'r') as f:
    content = f.read()

phase5c = '''\n### Phase 5C — Notes external & edge cases
- [x] notes-changed handler: refresh recent list; if open note file changed on disk and editor has no unsaved changes, reload content (keep caret position); if editor has unsaved changes, keep local text (next autosave overwrites); if file deleted/renamed outside Float, switch to draft keeping current text and save as new file on the next change
- [x] Settings: "Notes folder" row shows real path, shortened with middle ellipsis if long (full path in tooltip); "Open folder" calls notes_open_folder; no folder picker yet (Phase 8); to change folder edit notesFolder value in settings.json
- [x] Folder deleted while running: recreate on next save
- [x] Read-only or locked file: show "Couldn't save" in red (#ff453a); keep text in editor; retry on the next change
- [x] Very long titles: slug capped at 50 chars; trailing dots trimmed
- [x] Note over 2 MB: listed but opening shows "Too large to open here" with Open folder button
- [x] CRLF and UTF-8 BOM files: normalized on read; title extraction works correctly
- [x] Updated docs/FEATURES.md (Notes), docs/PROGRESS.md, and docs/TEST_CHECKLIST.md with manual Notes tests
- [x] pnpm build, front-end tests, cargo check and cargo test all pass'''

if '### Phase 5B' in content:
    # Insert after Phase 5B
    idx = content.find('### Phase 5B')
    # Find the end of Phase 5B section
    idx_end = content.find('### Phase 5C', idx+1)
    if idx_end < 0:
        # Find the next major section
        idx_end = content.find('\n### ', idx+1)
    if idx_end < 0:
        idx_end = len(content)
    
    content = content[:idx_end] + phase5c + content[idx_end:]
    with open('docs/PROGRESS.md', 'w') as f:
        f.write(content)
    print('Success: Phase 5C added to PROGRESS.md')
else:
    print('Phase 5B not found in PROGRESS.md')
    # Just append at the end
    content += phase5c
    with open('docs/PROGRESS.md', 'w') as f:
        f.write(content)
    print('Success: Phase 5C appended to PROGRESS.md')