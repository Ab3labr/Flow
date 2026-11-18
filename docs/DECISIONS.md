# Owner Decisions

Window behavior
- Window height follows panel height + 40 px bleed (grow before expand, shrink after close). Never leave a large transparent window capturing clicks.
- Outside-click close: global low-level mouse hook while pinned; window blur is a backup.
- Tray "Open Float" and "Settings" open the panel pinned and focused.
- Alt-menu suppression: inject an unassigned virtual key, not F24. Test Alt+Tab, Alt+F4, Alt+Space.

UI/UX
- Bundle Inter locally; no remote font imports.
- Hide inert controls (lyrics, Output device) in the real app until implemented.
- iCloud provider row hidden. ICS covers it.
- Calendar tab "+ Add" opens a new minimal add-event sheet; connecting calendars lives in Settings and the empty state.
- "Demo:" toggles exist in mock mode only.
- Recording pill gets a small hover-only stop button.
- Notes folder picker deferred to Phase 8. Webex matches webex.com. Recurrence: server-side for Google/Outlook, RRULE crate for ICS.

## Dependency justifications

Phase 1
- @tauri-apps/api ^2.12.5: Tauri 2 frontend bindings
- @tauri-apps/cli ^2.12.5: Tauri 2 CLI for dev/build
- windows ^0.62 (Win32_Foundation, Win32_System_Threading): single-instance enforcement via mutex (spec requirement)
- tauri-plugin-autostart: launch at startup (Phase 8)
