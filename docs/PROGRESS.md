# PROGRESS — Float (Tauri port)

Read this first if you're a new agent. Goal: turn the Figma Make prototype
into a real Windows app (Tauri 2 + Rust) without changing the design.

## Status legend
not started / in progress / done / blocked

## Phase checklist

### Phase 0 — Understand & document
- [x] Read codebase, write FEATURES/LAYOUT/ARCHITECTURE/PROGRESS + gap analysis
- [x] `git init` + initial commit (prototype + docs)
- [x] Update AGENTS.md for real project
- [x] Owner decisions recorded in docs/DECISIONS.md (8 questions resolved)

### Phase 1 — Tauri shell
- [x] `src-tauri/` scaffold (Cargo.toml, tauri.conf.json, build.rs,
      capabilities, icons, main.rs/lib.rs)
- [x] Window: frameless/transparent/always-on-top/skip-taskbar, ~560x400
- [x] Window positioning: top-center of cursor monitor, ignore-cursor-events when closed
- [x] Single instance (Windows mutex)
- [x] Tray icon: Open Float / Settings / Quit (`TrayIconBuilder`; emits `open-request`)
- [x] `settings.json` persistence (`settings_get`/`settings_set` commands)
- [x] SettingsView wired to real settingsService
- [x] `cargo check` + `pnpm tauri build` smoke test

### Phase 2 — Triggers
- [x] Rust cursor poll ~60 Hz, 240x4 px zone, 180 ms dwell, mouse-button +
      fullscreen suppression
- [ ] WH_KEYBOARD_LL hook thread: double-Alt 350 ms, custom shortcut parse,
      Alt menu suppression (F24 inject — verify)
- [ ] Pure functions + `cargo test` (dwell, alt-tap, shortcut)
- [ ] Frontend consumes `open-request`/`toggle-request` events

### Phase 3 — Music (SMTC)
- [ ] Now playing snapshot + events (session/props/playback/timeline)
- [ ] Art → data URL + sampled colors; duration null handling
- [ ] Transport commands + seek; position interpolation in UI
- [ ] Source label map; system volume (IAudioEndpointVolume)
- [ ] Idle home state when no session

### Phase 4 — Timer (Rust-authoritative)
- [ ] end-timestamp state machine, commands, `timer-state` events
- [ ] Toast + sound on finish; live pill integration
- [ ] Presets/custom/+1 min wired

### Phase 5 — Notes (markdown files)
- [ ] `%USERPROFILE%\Documents\Float Notes\*.md`, slug+timestamp names
- [ ] Debounced autosave + atomic write; recent-5 list; Recycle Bin delete
- [ ] Folder watcher → `notes-changed`; Open folder button

### Phase 6 — Calendar
- [ ] Local calendar CRUD (`calendar.json` or SQLite)
- [ ] ICS subscribe (15 min refresh, per-calendar color/visibility)
- [ ] Google OAuth PKCE + keyring (needs user client ID)
- [ ] Microsoft Graph OAuth PKCE + keyring (needs user client ID)
- [ ] docs/CALENDAR_SETUP.md
- [ ] Meeting-link detect (incl. Webex) + Join opens URL
- [ ] 5-min reminder → toast + live pill Join
- [ ] All design states: none/events/empty/error+reconnect

### Phase 7 — Capture
- [ ] Snip via `ms-screenclip:` URI
- [ ] Record via windows-capture (ffmpeg fallback) → MP4 in
      `Videos\Float Recordings`; red-dot pill w/ elapsed
- [ ] `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` on our window

### Phase 8 — Polish & release
- [ ] Launch at startup (autostart plugin honoring settings.launch)
- [ ] Hide-in-fullscreen trigger suppression verified
- [ ] Idle CPU <1%, no memory growth
- [ ] `.github/workflows/build.yml` (windows-latest: cargo test + tauri
      build + installer artifact)
- [ ] docs/TEST_CHECKLIST.md manual matrix
- [ ] docs/DECISIONS.md dep justifications (keep updated every phase)

## How to run

Prototype (browser, fake data):
```
pnpm install
pnpm dev          # vite dev server (Figma Make port env may apply)
VITE_USE_MOCK=1 pnpm dev   # explicit mock mode
```

Real app:
```
pnpm install
pnpm tauri dev    # builds Rust + vite, opens Float window
pnpm tauri build  # release bundle (NSIS installer on Windows)
```

Rust only:
```
cd src-tauri && cargo check && cargo test
```

## Environment notes
- Repo was NOT git-initialized at start; `git init` done in Phase 0.
- Toolchain pinned by `.mise.toml`: node 22, pnpm 10.34.3. Check
  `node -v`, `pnpm -v`, `cargo --version` before building.
- Machine is Windows — `cargo build` targets Windows natively; still keep
  Win32 code behind `#[cfg(windows)]` per spec.

## Known issues / blockers
- (none yet — update as phases land)

## Phase 0 summary
Prototype fully read: 14 source files, all fake data via
`src/services/mockServices.ts`. Docs written: FEATURES, LAYOUT,
ARCHITECTURE, GAP_ANALYSIS. Key decisions recorded in ARCHITECTURE
(window geometry model, service-layer target, event names).
Repo initialized; Phase 0 committed. Owner decisions resolved 8 open questions
via docs/DECISIONS.md. Tauri 2 deps installed (@tauri-apps/api, @tauri-apps/cli).

## Phase 1 summary
Tauri 2 shell scaffolded:
- Window configured: frameless, transparent, always-on-top, skip-taskbar, 560x400, decorations=false
- Single instance enforced via Windows mutex (com.float.app.windows)
- Settings file creation/load/save implemented (atomic temp+rename), stores NotchSettings, notesFolder, calendarAccounts, timerCustomTimes
- Commands: settings_get, settings_set
- Build succeeds (debug bundle produces MSI and NSIS installers)
- Deferred: window positioning to cursor monitor, ignore-cursor-events per state, tray icon with menu (Tauri 2 tray API complexity), SettingsView wiring

## Last session (settings persistence)
Done: `settingsService` backed by `settings_get`/`settings_set` (mock via
`VITE_USE_MOCK=1` or plain browser dev, persisted to localStorage). Notch
loads settings on mount and saves after every change; toggles and the
shortcut survive a restart. `notesFolder`, `calendarAccounts` and
`timerCustomTimes` are preserved on save. Demo toggles, the lyrics button and
the "Output device" row render only in mock mode.
Half-done / follow-ups:
- "Launch at startup" is persisted but the autostart plugin is still Phase 8.
- Calendar accounts/notes folder in the UI are not yet driven by settings.

## Last session (window model)
Done: `set_ui_mode(mode, panel_height)` command — idle (hidden +
click-through, 120x4 collapsed size), pill 200x90, open 560 x panel+40;
centered at y=0 on the monitor under the cursor, DPI-correct (tests at
100/125/150%). Startup mode is idle. Frontend reports mode on open/close/
tab/notes-height changes and delays idle by 250 ms after close.
`open-request` is consumed now (expands pinned, routes `{tab}`), and the real
app renders only `Notch` on a transparent body (fake desktop stays in the
browser prototype).
Half-done / follow-ups:
- Tray/shortcut opens do not set focus, only `pinned`; needs a source-aware
  `open-request` payload.
- The JS hover hot zone in `Notch` is inert while the window is hidden; the
  Rust hot zone is the real opener.
- Close delay is 250 ms while the designed close animation runs ~320 ms, so
  the window can hide just before the final animation frames.

## Last session (tray icon)
Done: `TrayIconBuilder` with "Open Float" / "Settings" / "Quit", icon from
`app.default_window_icon()`. Open Float emits `open-request` (null payload),
Settings emits `open-request {tab:"settings"}`, Quit calls `app.exit(0)`.
Half-done / follow-ups:
- Nothing consumes `open-request` yet, so the panel pin/focus and the
  Settings tab switch still need the frontend listener (Phase 2 item 4).

## Last session (Phase 2 hot zone)
Done: cursor poll thread (~60 Hz), 240x4 logical-px zone centered on the
cursor's monitor and scaled by that monitor's DPI, 180 ms dwell, mouse-button
and fullscreen suppression, emits `open-request`. Pure geometry/dwell/
suppression logic has 7 `cargo test` cases.
Half-done / follow-ups:
- Settings (`topEdge`, `hideInFullscreen`) are read once when the thread
  starts; changing them needs an app restart until settings-changed wiring.
- Fullscreen detection is a window-rect vs monitor-rect heuristic; verify
  borderless/maximized games on a real PC (Phase 8 item).
- `open-request` has no consumer yet (Phase 2 item 4) and no panel-state
  check, so it can fire while the panel is already open.

## Log
- Phase 0: docs written, git init, commit. Owner decisions resolved. Tauri deps installed.
- Phase 1: Tauri shell scaffolded, single-instance mutex, settings persistence, builds successfully.
- Phase 2: hot-zone cursor poll, dwell, suppression implemented with unit tests.
- Tray: TrayIconBuilder menu (Open Float/Settings/Quit) emitting `open-request`.
- Window model: `set_ui_mode` idle/pill/open, DPI-aware cursor-monitor placement, frontend open-request consumption.
- Settings: real settingsService (settings_get/settings_set), persisted toggles, mock-only demo/lyrics/output controls.
