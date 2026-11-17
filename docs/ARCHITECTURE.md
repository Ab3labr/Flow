# ARCHITECTURE

## Current prototype structure

```
index.html                  Vite shell, #root, figma placeholder comments
src/main.tsx                ReactDOM.createRoot → <App/> (StrictMode)
src/App.tsx                 fake desktop stage + <Notch/> + fake Taskbar
src/index.css               Tailwind v4 import, Inter @import, base layer,
                            .volume-range, .notch-scroll, reduced-motion
src/components/
  Notch.tsx                 ALL panel state + open/close + live pill + tab switching
  TopBar.tsx                tab bar, PressButton, TabId type
  CollapsedPill.tsx         Equalizer + unused CollapsedPill
  MusicCard.tsx             music card + … menu (volume)
  EventCard.tsx             home-screen event card + JoinButton
  TimerView.tsx             ring timer UI (HoldButton inside)
  CalendarView.tsx          today list + empty/error/connect states
  CalendarConnect.tsx       connect sheet + account rows
  NotesView.tsx             note editor + recent list + autosave debounce
  CaptureView.tsx           snip/record tiles
  SettingsView.tsx          toggles + shortcut capture + calendars + folder row
src/services/mockServices.ts  ALL data (tracks, accounts, events, notes,
                            timer presets/custom times, capture modes)
```

## State management
No store — `Notch.tsx` holds everything via `useState`/`useRef` and passes
props down. Timers are `window.setTimeout` refs. Services are synchronous
functions returning mock data.

## Service layer (today)
`src/services/mockServices.ts` exports `mediaService`, `calendarService`,
`timerService`, `captureService`, `noteService` plus shared types
(`Track`, `CalendarEvent`, `CalendarAccount`, `CalendarProvider`, `Note`).

- `mediaService.getTracks()` → static 3-track array (id, title, artist,
  source union, duration, colors triple).
- `calendarService`: `getDefaultAccounts`, `getProviders`,
  `createAccount`, `getToday(accounts)` (filters by visible+synced,
  formats `detail`/`time` strings, sorts), `getMeetingProvider(event)`.
- `timerService`: `getPresets` [5,10,25], `getCustomTimes` /
  `rememberCustomTime` (localStorage `notch-custom-times`, max 3 MRU).
- `captureService.getModes()` → ["Snip","Record"] labels.
- `noteService`: `list/get/create/save/remove` over localStorage
  `notch-notes`.

## Mock data locations
- Tracks: `tracks` array top of mockServices.ts.
- Accounts: `calendarAccounts` (google-work, outlook-work).
- Events: `rawEvents` + `mockEventStart = Date.now()` — times are relative
  to page load (5/80/150 min offsets).
- Notes/custom times: localStorage.
- Settings: `useState` in Notch (defaults topEdge/altAlt/fullscreen/launch/
  upcomingEvent true, noCalendars false, shortcut "Alt Alt") — NOT
  persisted.

## Planned service layer (target)
```
src/services/
  types.ts          shared DTOs (Track→NowPlaying, CalendarEvent w/ endsAt,
                    CalendarAccount, Note, NotchSettings, RecordingState…)
  index.ts          export mock or tauri impl by VITE_USE_MOCK
  mockServices.ts   current file, conformed to types.ts
  tauriServices.ts  invoke()/listen() wrappers — NEVER imported when mocking
```
New `settingsService` (get/set NotchSettings + notesFolder + calendar
account metadata via `settings_get`/`settings_set` commands).

## Planned Tauri architecture

### Crate layout
```
src-tauri/
  Cargo.toml          tauri 2 + plugins, windows-capture etc. per phase
  build.rs            tauri_build::build()
  tauri.conf.json     productName "Float", identifier com.float.app,
                      main window config (see below), bundle icons
  capabilities/       default.json permissions
  src/
    main.rs           entry → lib::run()
    lib.rs            Builder, plugins, setup (window, tray, threads)
    settings.rs       settings.json load/save (atomic)
    state.rs          shared AppState (settings handle, services)
    triggers.rs       hot-zone dwell + double-alt + shortcut (pure fns + OS)
    music.rs          SMTC (cfg windows) + trait + mock
    timer.rs          authoritative timer + toast + events
    notes.rs          markdown file store + watcher + recycle bin
    calendar/         local store, ics, oauth, google, msgraph, scheduler
    capture.rs        snip URI + graphics capture recording
```
All Win32/WinRT code behind `#[cfg(windows)]` traits with mock fallbacks so
`cargo check`/`cargo test` pass elsewhere.

### Window model (decision)
One always-on-top transparent window, `decorations:false`,
`alwaysOnTop:true`, `skipTaskbar:true`, `resizable:false`, sized ≈560x400
logical (max panel + shadow), horizontally centered on the monitor under
the cursor, y=0. Because it is centered for every width, panel
`left-1/2 -translate-x-1/2` stays screen-centered at any window width.

State-driven geometry + input (Rust owns, frontend reports):
- **idle/closed:** `set_ignore_cursor_events(true)` — clicks pass through.
  Window stays visible (avoids WebView2 suspend); verify idle CPU, else
  `hide()`.
- **pill active:** shrink window to ~200x90 centered, ignore=false.
- **open:** max size 560x400, ignore=false.

Frontend computes desired mode → emits `ui-mode` event payload
`{mode: "idle"|"pill"|"open"}`; Rust applies size/position/ignore. Rust
also shows window + emits `panel-state` for pill sources it owns (timer
done, recording, meeting reminder) so pills appear while the panel is
closed.

Intents frontend→Rust: `open-request` (from tray/second-instance),
`ui-mode`. Rust→frontend: `panel-state` (open/pill/tab hints),
`timer-state`, `media-state`, `notes-changed`, `calendar-changed`,
`recording-state`, `meeting-reminder`, `settings-changed`,
`open-request` (tray "Open Float"/"Settings").

Focus: `set_focus()` only for pinned/shortcut opens and when the Notes
tab needs typing; hover/pill opens do not steal focus.

Close-on-outside-click: webview can't see outside clicks → Rust watches
window blur (`Focused` event) → emit `close-request` → frontend `close()`.

### Triggers (Phase 2)
- Cursor poll thread ~60 Hz (`GetCursorPos`, `GetAsyncKeyState` for mouse
  buttons, foreground-window fullscreen check). Pure `DwellTracker`:
  open after 180 ms continuous dwell in 240x4 logical-px zone centered on
  monitor; suppressed while mouse down or (if enabled) fullscreen app
  foreground. All math in physical px using `scale_factor`.
- Keyboard hook thread: `WH_KEYBOARD_LL` + message loop. Pure
  `AltTapTracker` (350 ms window, any other key resets) + `Shortcut`
  parser for `settings.shortcut`. On Alt keydown inject F24 via
  `SendInput` to suppress menu-bar activation (verify on PC). Emits
  `toggle-request`.
- Unit tests: `cargo test` on pure trackers/parser.

### Persistence
`%APPDATA%\Float\settings.json` via `app.path().app_config_dir()`:
`NotchSettings` fields, `notesFolder`, `calendarAccounts` metadata
(id/provider/email/name/color/visible/status — never tokens),
`timerCustomTimes`, OAuth account list. Atomic write (temp+rename).

### Services → Rust commands (planned)
- media: `media_snapshot`, `media_transport(action)`, `media_seek(ms)`,
  `media_volume_get/set`; event `media-state`.
- timer: `timer_set/pause/resume/add/get`; event `timer-state`;
  expiry → toast + `timer-done`.
- notes: `notes_list/get/create/save/remove/open_folder`; event
  `notes-changed`.
- calendar: `calendar_accounts`, `calendar_connect(provider)`,
  `calendar_disconnect`, `calendar_toggle`, `calendar_refresh`,
  `calendar_add_event/update/delete`, `calendar_join(url)`; event
  `calendar-changed`.
- capture: `capture_snip`, `capture_record_start/stop`; event
  `recording-state`.
- settings: `settings_get`, `settings_set`.

### Multi-monitor / DPI
`GetCursorPos` = physical px; Tauri `Monitor::position/size` physical,
`scale_factor` for logical. Do trigger math in physical px; window
position/size in logical px via Tauri APIs (it converts).

### Tray
`TrayIconBuilder` menu: "Open Float" → `open-request`; "Settings" →
`open-request {tab:"settings"}`; "Quit" → exit. Icon: generated PNG in
`src-tauri/icons/`.

### Plugins (planned, each justified in docs/DECISIONS.md)
- `tauri-plugin-single-instance` (Phase 1)
- `tauri-plugin-notification` (Phase 4/6 toasts)
- `tauri-plugin-opener` or `open` crate (join links, folder, URIs)
- `tauri-plugin-autostart` (Phase 8)
- Rust crates: `serde/serde_json`, `windows` (SMTC, Core Audio, Win32),
  `keyring`, `oauth2`, `reqwest`, `notify`, `trash`, `time`/`chrono`,
  `windows-capture` (Phase 7).

## Frontend refactor notes
- `Notch.tsx` keeps ALL animation/JSX; swap data sources to services +
  event subscriptions. Remove its own Alt/hotzone handlers when Rust
  triggers land (keep Esc + outside-pointerdown for mock mode).
- `App.tsx`: render `<Notch/>` on transparent body; keep fake desktop
  only behind a dev flag if useful.
- `index.html`: `<title>Float</title>`.
- `vite.config.ts`: verify Figma Make plugins don't break
  `pnpm tauri dev`/`build`; `devUrl`/`frontendDist` wiring.
- `VITE_USE_MOCK=1` → mock services, no `@tauri-apps/api` import.
