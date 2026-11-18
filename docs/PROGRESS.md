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
- [ ] Tray icon: Open Float / Settings / Quit (`TrayIconBuilder`; emits `open-request`)
- [ ] `settings.json` persistence (`settings_get`/`settings_set` commands)
- [x] SettingsView wired to real settingsService
- [x] Inter bundled locally (@fontsource-variable/inter), no remote font import
- [x] `cargo check` + `pnpm tauri build` smoke test

### Phase 2 — Triggers
- [x] Rust cursor poll ~60 Hz: `in_hot_zone` in physical px (240x4 × monitor
      scale), 180 ms dwell, mouse-button + fullscreen suppression (shell
      classes excluded), emits `open-request {source:"hover"}` only while the
      panel is idle
- [x] WH_KEYBOARD_LL hook thread: double-Alt 350 ms (`AltTapTracker`,
      emits `toggle-request {source:"alt"}`)
- [ ] Custom shortcut parse (JS handler still covers focused-window combos)
- [ ] Alt menu suppression (inject an unassigned virtual key — see
      DECISIONS.md); testable only after the hook is verified without it
- [x] Pure functions + `cargo test` (dwell, alt-tap)
- [x] Frontend consumes `open-request`/`toggle-request` events
- [x] Settings live via `RwLock` shared state (`settings_set` applies with no
      restart), suppression-reason logging ≤1/s, `settingsVersion` migration

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
- Deferred: window positioning to cursor monitor, ignore-cursor-events per state, tray icon with menu (Tauri 2 tray API complexity), SettingsView wiring

## Last session (live settings & unmistakable toggles)
Done: `settingsVersion` (current 2) on `AppSettings` (`#[serde(default)]` → 0
when missing). `settings::init` runs once in setup after the file exists:
`load_or_default` (missing/corrupt → all defaults), then
`apply_version_migration` resets `topEdge`, `altAlt`, `hideInFullscreen` and
`launchAtStartup` to true when the version is missing/older, saves the file
and sets the version. `settings_set` forces the version to 2 so a frontend
payload cannot re-trigger the migration.
Live state: `static LIVE: LazyLock<RwLock<NotchSettings>>` seeded by `init`;
`top_edge()`, `alt_alt()` and `hide_in_fullscreen()` read it on every trigger
cycle/event, and `settings_set` replaces it after a successful save. The
hot-zone thread no longer exits when `topEdge` is off, and the Alt hook is
always installed and checks `altAlt` per event — both toggles now apply with no
restart. `settings_get` refreshes the live state too.
Logging: one Info line with the four values at startup (`settings: topEdge=…
altAlt=… hideInFullscreen=… launchAtStartup=…`) and on every successful
`settings_set`. Hover suppression logs an Info reason while the cursor is in
the zone — `top edge disabled in settings`, `mouse button held` or `fullscreen
app detected` — immediately on a change and otherwise at most once per second.
`Toggle`: one `value` drives track, knob position/colour and the row label.
ON = `#FFFFFF` track, `#000000` knob right, label `text-white`; OFF = `#2a2a2a`
track with a 1 px `#444444` inset outline, `#8a8a8a` knob left, label
`text-white/50`; OFF hovers to `#343434`. Size (36x20, knob 14) and the 500/32
spring are unchanged; `role="switch"`, `aria-checked`, Space/Enter via the
native button, and a `focus-visible` ring. `Notch.tsx` saves through
`updateSettings`: optimistic `setSettings`, then `settingsService.save`, and on
failure reverts to the previous object and logs (a sequence counter ignores
stale failures). The old `[settings]` auto-save effect is gone.
Half-done / follow-ups:
- Optimistic save has no visible error toast; failures only appear in the
  console (the toggle snaps back).
- `settingsVersion` is not typed on the TypeScript `AppSettings`; it is
  preserved by the spread in `tauriServices` and forced by Rust anyway.
- Fullscreen heuristic and Alt-menu suppression are unchanged from the
  previous sessions.

## Last session (settings toggles & defaults)
Done: one shared `Toggle` in `SettingsView` drives the track, the knob and
`aria-checked` from the same `value` boolean. ON = white track + black knob at
x 19; OFF = `#303030` track + `#9a9a9a` knob at x 3. Size (36x20, knob 14),
positions and the 500/32 spring are unchanged.
Defaults are `topEdge`, `altAlt`, `hideInFullscreen` and `launchAtStartup` all
true in both TypeScript (`DEFAULT_NOTCH_SETTINGS`) and Rust (`DEFAULT_SETTINGS`
plus a per-field `#[serde(default = "…")]` for every notch field, so a
settings.json missing a field loads with the default instead of failing).
Legacy keys `"fullscreen"`/`"launch"` are ignored (serde behaviour, covered by
a unit test). Frontend and Rust use the same names (`topEdge`, `altAlt`,
`hideInFullscreen`, `launchAtStartup`, `shortcut`); `settingsService` passes the
notch object straight through in both directions.
Half-done / follow-ups:
- Existing settings.json files keep their saved values; the new defaults only
  apply to missing fields and fresh installs.
- "Launch at startup" still only persists — the autostart plugin is Phase 8.

## Last session (hot-zone rewrite)
Done: the top-edge trigger is now physical-px only. `in_hot_zone(cx, cy, mon_x,
mon_y, mon_w, scale)` computes the 240x4 design zone × the monitor's scale
factor (1.0/1.25/1.5 covered on 1920 and 2560 wide monitors, second monitor
offsets incl. negative, half-open edges, top-edge anchoring). `DwellTracker`
is now `new(ms)` + `update(inside, suppressed, now)` and resets on
leave/suppress; tests cover too-short, exactly 180 ms, once per entry, and a
suppression restart. The Windows glue polls every 16 ms, takes the monitor
under the cursor (position/width/scale from that monitor), suppresses on L/R/M
held or `hideInFullscreen` + a real fullscreen window (rect equals the whole
monitor rect and class is not Progman/WorkerW/Shell_TrayWnd, so maximized and
desktop do not count). It emits `open-request {source:"hover"}` only while the
panel is closed (`window_mode::is_closed()` = last mode idle) and logs cursor /
zone / scale / inside / suppressed once per second at debug level, plus a line
on fire. Missing `topEdge` disables the thread at startup.
Frontend: the JS 1 px hot-zone strip returns early in Tauri; `open-request`
payloads are source-aware — `hover` opens unpinned (400 ms leave-close still
applies), tray opens stay pinned.
Half-done / follow-ups:
- The 1 s diagnostics are at debug level while the global filter is Info, so
  they only appear after raising the level (fire/suppression lines are Info).
- `topEdge` / `hideInFullscreen` are still read once at thread start.
- Fullscreen detection assumes a true-fullscreen window matches the monitor
  rect exactly; verify borderless games and DPI changes on the PC (Phase 8).
- `is_closed` trusts the last `set_ui_mode` call; if the frontend never
  reports (e.g. crashed webview) hover stays suppressed until it does.

## Last session (double-Alt hook)
Done: double-Alt moved into Rust. `AltTapTracker` (std only, pure) handles
AltDown/AltUp/OtherDown with a 350 ms window, ignores auto-repeat, and is
unit-tested (7 cases: in-window fire, 500 ms apart, Alt+Tab, key between taps,
auto-repeat, third tap, stray AltUp). A dedicated thread installs
`WH_KEYBOARD_LL` with `SetWindowsHookExW` and runs a `GetMessageW` loop.
All four messages (WM_KEYDOWN/WM_KEYUP/WM_SYSKEYDOWN/WM_SYSKEYUP) are handled,
Alt is 0x12/0xA4/0xA5, `LLKHF_INJECTED` events are dropped, and every event is
passed on with `CallNextHookEx`. A fire emits `toggle-request {source:"alt"}`.
`altAlt` is read once at thread start (like `topEdge`). Every Alt event and
every fire is logged to `float.log` (log plugin now runs in release too and
writes the `float` file name instead of `Float`).
Frontend: the JS double-Alt sequence is disabled when `isTauriApp` (kept for the
browser prototype); `toggle-request` opens pinned + focused (focus 80 ms after
`set_ui_mode` shows the window) or closes when already open.
Half-done / follow-ups:
- No Alt-menu suppression yet (by design: test this first). Double-Alt may
  still flash/focus the window menu; suppression injects an unassigned VK
  (never F24) per DECISIONS.md.
- `altAlt` is read only at startup; toggling it needs a restart until a
  settings-changed hook exists.
- Custom shortcuts (non "Alt Alt") still come from the JS handler and only work
  while the panel has focus.

## Last session (settings naming)
Done: TypeScript and Rust now share the same names and defaults —
`hideInFullscreen` and `launchAtStartup` everywhere, defaults topEdge/altAlt/
hideInFullscreen true and launchAtStartup false. The mapping layer is gone;
`settingsService` passes the notch object straight to `settings_set`.
Settings tab changes still save to `settings.json` and load on startup. Demo
rows stay mock-only; lyrics and "Output device" stay hidden in the real app.

## Last session (font + service split)
Done: Tauri renders only `Notch` on a fully transparent page (no wallpaper,
taskbar or 1920x1080 stage scaling); a plain browser keeps the fake desktop.
Inter is bundled via `@fontsource-variable/inter`, the Google Fonts import is
gone, and the Tauri CSP is `font-src 'self'`. Services split into `types.ts`,
`mockServices.ts`, `tauriServices.ts` (settings only) and `index.ts`;
`VITE_USE_MOCK=1` forces mock and a plain browser defaults to mock.
Half-done / follow-ups:
- Media/calendar/timer/notes still have mock implementations only; real ones
  land with their phases.
- `tauriServices.ts` is statically imported (so bundled) in mock mode too;
  only its calls are gated by `VITE_USE_MOCK`.

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
- Fonts/services: Inter bundled locally, Tauri-only notch render, services split into types/mock/tauri/index.
- Settings: unified `hideInFullscreen`/`launchAtStartup` names and defaults across Rust/TypeScript.
- Triggers: Rust WH_KEYBOARD_LL double-Alt hook (`toggle-request`), JS sequence Tauri-gated, pure tracker unit-tested.
- Triggers: physical-px `in_hot_zone` + new `DwellTracker`, closed-only `open-request {source:"hover"}`, unpinned hover open.
- Settings: single-value `Toggle` (white/black on, #303030/#9a9a9a off), four toggles default true, serde defaults for partial settings.json.
- Settings: `settingsVersion` 2 migration, live `RwLock` state for both triggers, suppression-reason logs, redesigned toggle (white/#000 on, #2a2a2a/#8a8a8a off, dimmed label, focus ring), optimistic save + revert.
