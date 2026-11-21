# ARCHITECTURE

## Current prototype structure

```
index.html                  Vite shell, #root, figma placeholder comments
src/main.tsx                ReactDOM.createRoot → <App/> (StrictMode)
src/App.tsx                 fake desktop stage + <Notch/> + fake Taskbar
src/index.css               Tailwind v4 import, Inter @import, base layer,
                            .volume-range, .notch-scroll, reduced-motion
src/components/
  Notch.tsx                 panel state + open/close + activity registration + tabs
  TopBar.tsx                tab bar, PressButton, TabId type
  MusicCard.tsx             music card + … menu (volume)
  EventCard.tsx             home-screen event card + JoinButton
  TimerView.tsx             ring timer UI (HoldButton inside)
  CalendarView.tsx          today list + empty/error/connect states
  CalendarConnect.tsx       connect sheet + account rows
  NotesView.tsx             note editor + recent list + autosave debounce
  CaptureView.tsx           snip/record tiles
  SettingsView.tsx          toggles + shortcut capture + calendars + folder row
src/live/
  types.ts                  LiveActivity contract + leading/trailing/actions
  priorities.ts             shared activity priorities
  pickActivity.ts           pure transient-first priority picker
  useLiveActivities.ts      external store, register/update/remove, TTL expiry
  CompactBar.tsx            single compact renderer, content-fit width
  Equalizer.tsx             3-bar playing/paused indicator
  geometry.ts               pure bar/idle/window-margin constants
  windowTransition.ts       pure surface→window plan (expand/collapse/stable)
  useWindowTransition.ts    one start + one final set_ui_mode per morph
src/services/mockServices.ts  ALL data (tracks, accounts, events, notes,
                            timer presets/custom times, capture modes)
```

## State management
`Notch.tsx` holds panel/tab state via `useState`/`useRef` and passes props
down. Live activities live in a small `useSyncExternalStore` module with
stable `register`/`update`/`remove` functions and per-transient expiry timers;
`pickActivity` is pure. Other timers are `window.setTimeout` refs. Services are
synchronous functions returning mock data.

## Service layer (today)
`src/services/mockServices.ts` exports `mediaService`, `calendarService`,
`timerService`, `captureService`, `noteService` plus shared types
(`Track`, `CalendarEvent`, `CalendarAccount`, `CalendarProvider`, `Note`).

- `mediaService.getTracks()` → static 3-track array (id, title, artist,
  source union, duration, colors triple).
- `calendarService`: `getDefaultAccounts`, `getProviders`,
  `createAccount`, `getToday(accounts)` (filters by visible+synced,
  formats `detail`/`time` strings, sorts), `getMeetingProvider(event)`.
- `timerService` (real in `tauriServices.ts`, mock in browser):
  `getPresets` [5,10,25], `get`/`subscribe` (full `TimerSnapshot`),
  `set`, `start`, `pause`, `resume`, `reset`, `adjust`, `restart`. Snapshot
  `{state,durationMs,totalMs,remainingMs,capturedAtMs,endsAtMs?,
  lastCustomMs}`; commands return it and Rust also emits `timer-state`.
  `src/live/useTimer.ts` subscribes and counts `remainingMs` down locally every
  250 ms from `endsAtMs`. Mock keeps the same wall-clock machine with
  localStorage duration/last-custom keys.
- `captureService.getModes()` → ["Snip","Record"] labels.
- `noteService`: `list/get/create/save/remove` over localStorage
  `notch-notes`.

## Mock data locations
- Tracks: `tracks` array top of mockServices.ts.
- Accounts: `calendarAccounts` (google-work, outlook-work).
- Events: `rawEvents` + `mockEventStart = Date.now()` — times are relative
  to page load (5/80/150 min offsets).
- Notes/custom times: localStorage.
- Settings: mock values persisted to localStorage; real values via
  `settingsService` (`settings_get`/`settings_set`).

## Service layer
```
src/services/
  types.ts          shared DTOs (Track, CalendarEvent, CalendarAccount,
                    Note, NotchSettings)
  index.ts          exports mock or tauri impl by VITE_USE_MOCK
  mockServices.ts   mock data + localStorage impl
  tauriServices.ts  invoke()/listen() wrappers — settings, media, power, timer
```
`settingsService` is real (NotchSettings + notesFolder + calendar account
metadata via `settings_get`/`settings_set`; unrelated fields preserved).
`VITE_USE_MOCK=1` forces mock; a plain browser defaults to mock.

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
    power.rs          GetSystemPowerStatus thread (cfg windows) + pure decision fns
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

State-driven geometry + input (Rust owns, frontend reports via the
`set_ui_mode(mode, phase, panel_height, bar_width, bar_height, pinned, logical_open)` command):
- **idle:** `set_ignore_cursor_events(true)` and hidden.
- **pill:** `(bar width + 32) x (bar height + 18)` logical, centered on the top
  edge, input accepted. The single compact live bar is 120-260 x 30 (or the
  activity's `maxWidth`); the window mode keeps its historical `pill` name.
- **open:** 560 x (panel height + 40) logical, input accepted.
- **phase:** `stable` applies the real mode normally. `expanding` grows to the
  target geometry before Motion runs. `collapsing` holds the source geometry
  until Motion settles, then the frontend sends the final stable plan. Both
  transitional phases set `set_ignore_cursor_events(true)` so the temporary
  larger window cannot capture clicks. Missing phases are treated as `stable`.
- **logical_open:** the target panel state, separate from the physical `mode`
  held during a collapse. Triggers (double-Alt, Esc, outside click) read this,
  so a quick gesture right after closing reopens instead of closing again.
  Optional for older frontends; defaults to `mode == "open"`.

Every mode is horizontally centered, y=0, on the monitor under the cursor,
computed in physical px with that monitor's `scale_factor`. Rust drops repeated
applies with identical geometry/phase/input state; the frontend also dedupes
commands. The full panel and compact bar are overlapping layers of one
container, so there is no intermediate idle sliver between them. Startup mode
is idle. Rust still owns `panel-state` for future pill sources; today the
frontend owns transient TTLs.

Startup handshake: every Tauri listener registers through
`services/listenerRegistry.ts`; once all of them settle the frontend calls the
`ui_ready` command, Rust resets to idle/stable/`logical_open=false`, resets the
Alt tracker and flushes triggers queued before readiness (at most one per kind,
newest wins). The first plan after `ui_ready` is force-sent past the planKey
dedupe, so both sides are guaranteed to agree. An emitted `open-request` arms a
400 ms watchdog: if no plan arrives, Rust logs a warning, applies the visible
open geometry (payload `pinned` preserved) and re-emits the event once.

Intents frontend→Rust: `set_ui_mode` command (geometry/input/pinned), settings
commands. Rust→frontend: `open-request` (tray/hot zone/double-Alt),
`close-request` (double-Alt/Esc/outside click),
`panel-state` (open/pill/tab hints),
`timer-state`, `media-state`, `power-state`, `notes-changed`,
`calendar-changed`, `recording-state`, `meeting-reminder`, `settings-changed`.

Focus: `set_focus()` only for pinned/shortcut opens and when the Notes
tab needs typing; hover/pill opens do not steal focus.

Close-on-outside-click: `WH_MOUSE_LL` (own thread + message loop) reads the
Rust-owned panel rect; an outside L/R/M down while the panel is open *and
pinned* emits `close-request {source:"outside"}`. The click is never swallowed.
Hover-opened panels stay unpinned and keep the 400 ms leave-close.
Rust owns `panel_open` / `panel_pinned` / panel rect (`AppState`), updated by
`set_ui_mode`.

### Triggers (Phase 2)
- Cursor poll thread ~60 Hz: `GetCursorPos` (physical px), the monitor under
  the cursor contributes its position/width/scale, `GetAsyncKeyState` (L/R/M)
  and a foreground-window fullscreen check. Pure `DwellTracker`: fires once
  after 180 ms continuous dwell in the 240x4 physical-px zone (`in_hot_zone`,
  design size × monitor `scale_factor`); inside + not suppressed resets the
  clock. Suppressed while a mouse button is held or (if `hideInFullscreen`) a
  fullscreen app is in front. Fullscreen means the foreground window's rect
  equals the whole monitor rect and its class is not Progman / WorkerW /
  Shell_TrayWnd, so maximized windows and the desktop never count.
- Hot zone emits `open-request {source:"hover", pinned:false}` only while the
  panel is idle. The frontend opens it unpinned, so the 400 ms leave timer
  still closes it. The JS 1 px hot-zone strip is browser-prototype only.
- Keyboard hook thread: `WH_KEYBOARD_LL` + message loop. Pure
  `AltTapTracker` (350 ms window, any other key resets) + `Shortcut`
  parser for `settings.shortcut`. On double-Alt Rust decides from
  `AppState.panel_open`: `close-request {source:"alt"}` or
  `open-request {source:"alt", pinned:true}`. Escape closes an open panel
  (`close-request {source:"esc"}`); both keys always pass through. The tracker
  is reset whenever the panel opens or closes so a missed Alt-up cannot stick.
- Mouse hook thread: outside-click close as described above.
- Alt-menu suppression (inject an unassigned virtual key, not F24) is still
  pending; verify on PC with Alt+Tab, Alt+F4, Alt+Space.
- Unit tests: `cargo test` on pure trackers/parser/rects.

### Music / now playing (Phase 3A/3B)
One dedicated `float-media` thread calls `RoInitialize(RO_INIT_MULTITHREADED)`
once, then polls SMTC every 500 ms:
`GlobalSystemMediaTransportControlsSessionManager::RequestAsync()` (kept for
the thread's lifetime). Session choice is sticky: the shown session while it
keeps playing, else `GetCurrentSession()` if playing, else the first playing
session, else the paused current session, else none. Switches log the AUMID.
Per session: media properties (title/artist), app AUMID, playback status and
rate, timeline (`TimeSpan` is 100 ns → `/10_000` ms; duration null when end ≤
start). `TimelineProperties::Position()` is corrected by
`(now - LastUpdatedTime) * PlaybackRate` while playing (zero/future timestamps
ignored, clamped to duration); `playbackRate` is in the payload. Thumbnail
bytes are read through `DataReader` and cached by title+artist; >1.5 MB is
skipped. Art becomes a `data:<content-type>;base64,…` URL; a tiny local base64
encoder avoids a dependency. The windows 0.62 blocking wait is
`IAsyncOperation::join()` (not `.get()`), and it only ever runs on this thread.

`media_snapshot` returns the cached `{title, artist, source, playing,
playbackRate, positionMs, durationMs, artDataUrl, capturedAtMs}` (or null).
`media-state` is emitted when title, artist, source, playing, rate or duration
changes, when the chosen session changes, when the session disappears (one
`null`), and when the UI's projected position drifts more than 1.5 s from the
corrected one (seek). Otherwise the UI interpolates from `positionMs` +
`capturedAtMs` × rate. The app AUMID is mapped to Spotify / Apple Music /
Chrome / Edge / Brave / Firefox / Opera, otherwise the readable AUMID/exe name.

Transport: `media_toggle`, `media_next`, `media_prev`, `media_seek(positionMs)`
send a message over a std mpsc channel to the music thread and wait (on a
blocking task, 2 s timeout) for a bool. The thread resolves the shown session by
AUMID and calls `TryTogglePlayPauseAsync`, `TrySkipNextAsync`,
`TrySkipPreviousAsync` or `TryChangePlaybackPositionAsync(ms * 10_000)`. After
a command it polls every 150 ms for 1.5 s.

### Power / charging (Phase 3D-4)
One `float-power` thread calls `GetSystemPowerStatus` every 500 ms. Pure
`decide()` keeps the silent startup baseline and emits
`power-state` `{pluggedIn, charging, percent, transition}` on the same poll
that sees a changed AC or percent signal. After an emitted AC change it starts an 800 ms
flicker guard: further AC flips are held and the final reading wins with one
corrected event. Percent-only changes emit `transition: "none"` immediately
and never move the AC guard. A charging-bit-only change is not part of the
signal, so it never emits; no battery or unknown AC resets the baseline
silently. `FLOAT_DEBUG=1` logs a timestamp/delta for change detection and
emission (and sets `window.FLOAT_DEBUG` so the webview logs its own stages).

### Persistence
`%APPDATA%\Float\settings.json` via `app.path().app_config_dir()`:
`NotchSettings` fields, `notesFolder`, `calendarAccounts` metadata
(id/provider/email/name/color/visible/status — never tokens),
`timerCustomTimes`, `timerDurationMs`, `timerLastCustomMs`, `timerSound`,
OAuth account list. Atomic write (temp+rename).

### Services → Rust commands (planned)
- media: `media_snapshot`, `media_toggle/next/prev/seek` (done);
  event `media-state` (done). System volume still Phase 3B+/8.
- timer (Phase 4A, done): `timer_get`, `timer_set`, `timer_start`,
  `timer_pause`, `timer_resume`, `timer_reset`, `timer_adjust`,
  `timer_restart`; each returns and emits `timer-state {state, durationMs,
  totalMs, remainingMs, capturedAtMs, endsAtMs?}`. A 250 ms thread ticks the
  wall-clock state machine; completion emits `timer-done {totalMs}`, plays
  three MessageBeeps and shows a native toast. Settings gain
  `timerDurationMs` / `timerLastCustomMs` / `timerSound`; a running timer is
  not saved across restarts. UI wiring is Phase 4B.
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
`TrayIconBuilder` menu: "Open Float" → `open-request` (opens panel pinned and focused); "Settings" →
`open-request {tab:"settings"}` (also pinned and focused); "Quit" → exit. Icon: generated PNG in
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
