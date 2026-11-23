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
- [x] WH_KEYBOARD_LL hook thread: double-Alt 350 ms (`AltTapTracker`);
      Rust decides open/close and emits `open-request`/`close-request`
- [ ] Custom shortcut parse (JS handler still covers focused-window combos)
- [ ] Alt menu suppression (inject an unassigned virtual key — see
      DECISIONS.md); testable only after the hook is verified without it
- [x] Pure functions + `cargo test` (dwell, alt-tap)
- [x] Frontend consumes `open-request`/`close-request` events
- [x] Settings live via `RwLock` shared state (`settings_set` applies with no
      restart), suppression-reason logging ≤1/s, `settingsVersion` migration
- [x] Rust-owned panel state: double-Alt open/close decision, Esc close,
      outside-click close while pinned (`close-request`), tracker reset

### Phase 3 — Music (SMTC)
- [x] Now playing snapshot + events (session/props/playback/timeline)
- [x] Art → data URL keyed by title+artist+album, retried every 1 s for 15 s
      while playing; >400 KB downscaled to ≤256 px JPEG; 8x8 canvas glow;
      duration null handling
- [x] Position corrected from `LastUpdatedTime` + `PlaybackRate`; seek resync
      (>1.5 s drift)
- [x] Session priority: music app > other > browser, longest playing wins,
      ties use `GetCurrentSession()`; paused falls back to most recent
- [x] Transport commands + seek (`media_toggle/next/prev/seek`, 150 ms fast
      polling for 1.5 s); commands target only the displayed session object
- [x] Position interpolation in UI from `positionMs`/`capturedAtMs`/rate;
      whole-second labels floored so elapsed + remaining = duration
- [x] Source label map
- [ ] System volume (IAudioEndpointVolume) — dropped from the UI
- [x] Idle home state when no session

### Phase 3D — Compact live bar
- [x] Reusable `src/live` system: `LiveActivity` slots, priority picker
      (recording 100 / privacy 97 / charging 95 / timer done 90 / meeting 80 /
      timer 60 / music 10), external store with register/update/remove, one
      `CompactBar` renderer
- [x] Music activity while playing; paused keeps flat bars for 3 s then
      removes; no session means no activity
- [x] Compact-bar gate from Rust `isMusicApp` (browser album heuristic);
      full music card still shows any SMTC session
- [x] Bar look/motion: 30 high, 120-260 wide, radius 15, dominant-colour glow,
      one 380/30/0.9 spring for every direction, 6 px blur fade, 260 ms title
      crossfade, 150 ms reduced-motion fades
- [x] Hover -> Home unpinned, click -> pinned, close while playing -> bar;
      fullscreen suppression; `compactBar` setting applies live
- [x] `set_ui_mode` pill takes frontend bar size: (width + 32) x (height + 18)
- [x] Migrate recording/timer/reminder onto the same system; remove the old
      pill renderer and branches (Phase 3D-3)
- [x] Activity contract adds `mode`, `ttlMs`, `hover`, `actions`, `expandTab`,
      `maxWidth` and optional `render`; transient-first picker + TTL removal
      with fallback to the best persistent activity
- [x] Timer Start / Capture Record dissolve the panel into the bar after
      450 ms when that compact toggle is on; closing with a persistent
      activity alive returns to the bar
- [x] "COMPACT BAR" settings: master `compactBar` plus `compactMusic`,
      `compactTimer`, `compactRecording`, `compactCharging`; all default true
      in TypeScript and Rust (`#[serde(default = "default_true")]`)
- [x] Phase 3D-4: Windows `GetSystemPowerStatus` thread (500 ms poll, immediate
      emit on change, 800 ms flicker guard), pure transition logic with
      no-battery/unknown-value tests, and `power-state` service event
- [x] Charging compact activity: transient/95/4000 ms, bolt or battery leading
      icon (red at ≤20%), "Charging"/"Charged"/"Battery", trailing percentage;
      percent updates refresh a visible slot without extending the TTL and never
      open the bar on their own; `compactCharging` applies live

### Phase 3D-5B — Full notch ↔ compact bar transition
- [x] One visual container: panel and bar are overlapping layers in the same
      motion.div; no `mode="wait"` blank frame and no idle sliver between them
- [x] Pure transition planner holds the source geometry while collapsing and
      grows before expanding; retargeting handles reversed gestures
- [x] Rust `set_ui_mode` accepts an explicit `expanding`/`collapsing`/`stable`
      phase, skips duplicate applies, and keeps the morphing window click-through
- [x] Single final resize per morph after the spring settles; idle shrink/hide
      and compact width changes follow the same path
- [x] `pnpm test` expanded to 16 frontend tests; Rust adds phase parsing test
- [x] Follow-up fixes: center the compact bar inside the morphing container so
      it does not slide during a width change; keep Rust's logical `open` state
      tied to the target surface so a quick double-Alt during a collapse reopens

### Phase 4 — Timer (Rust-authoritative)
- [x] Pure end-timestamp state machine (`Idle`/`Running`/`Paused`/`Done`, Unix
      ms) with fake-clock tests; backwards clock cannot extend it
- [x] Commands `timer_get/set/start/pause/resume/reset/adjust/restart`; each
      returns and emits `timer-state` (no per-second events; `endsAtMs`)
- [x] Toast + 3x MessageBeep alert on finish (`tauri-plugin-notification`);
      failures logged without breaking the timer
- [x] Persistence: `timerDurationMs` (25 min default), `timerLastCustomMs`
      (non-preset starts), `timerSound`; running timer not saved across restarts
- [x] Compact-bar integration with the Rust state (`timer-active`/`done`
      derived from `useTimer()`; start collapses only with `compactTimer`)
- [x] New single-focus TimerView wired: idle digits/chips, running/paused/done,
      Space, progress + "Ends", reset/primary; custom chip from
      `lastCustomMs` (the old ring/drag/custom/+1-min UI is gone)
- [x] Timer activity derived from real timer state: exists only while running;
      paused/idle/done remove it. Leading: small progress ring; label: remaining
      time counted locally from `endsAtMs`. Priority 60, hover "expand".
- [x] "Timer done" from `timer-done` event (5 s, priority 90): works regardless
      of `compactTimer`, no compactBar master toggle needed for sound/toast.
- [x] `compactTimer` semantics: ON (default): Start collapses panel into bar
      with countdown. OFF: nothing about timer appears in bar while running;
      panel stays open after Start; on completion still gets toast, sound and
      "Timer done" message.
- [x] Settings: "Timer countdown" row in COMPACT BAR section with caption
      "Turn a row off to keep it out of the bar." Added small "TIMER" section
      with "Timer sound" toggle (shared Toggle), applies live.
- [x] Tests: running adds activity, pausing removes it, done removes it and
      raises transient, `compactTimer` off removes activity.

### Phase 5 — Notes (markdown files)
- [x] Pure functions: `parse_note`, `serialize_note`, `slugify`, `file_name`
  - CRLF→LF, BOM strip, title extraction, body with one leading blank line removed
  - Forbidden char removal, whitespace collapse, dot trimming, 50-char cap, empty→"Untitled"
  - `file_name(datetime, slug)` → "YYYY-MM-DD HH-mm-ss {slug}.md"; collision helper
- [x] File I/O operations: `notes_dir`, `notes_list`, `notes_get`, `notes_save`, `notes_delete`, `notes_open_folder`
  - notes_dir uses settings.notesFolder or Tauri `documents_dir()` + "Float Notes"; create if missing
  - notes_list: sort by mtime newest first, return {id, title, modifiedMs} reading only first 512 bytes
  - notes_get: reject >2 MB files
  - notes_save: create if null, atomic write (temp + rename), rename if slug changes from "Untitled" to non-empty
    never create when both title and body empty
  - notes_delete: trash::delete to Recycle Bin
  - notes_open_folder: create folder then explorer.exe
- [x] Folder watcher with notify crate: non-recursive, 150 ms debounce, 500 ms own-write ignore; emit "notes-changed"
- [x] Flush on quit: tray "Quit" emits "flush-request" waits up to 800 ms for "flush-done" from front end, then exits
- [x] Tauri commands: notes_list, notes_get, notes_save, notes_delete, notes_open_folder, notes_folder_path
- [x] `trash` crate dependency added with justification in docs/DECISIONS.md
- [x] `settingsService` integration for `notesFolder` setting
- [x] All `cargo test` (112 passing), `cargo check` clean, `pnpm build` succeeds

### Phase 6 — Calendar
- [ ] Local calendar CRUD (`calendar.json` or SQLite)
- [ ] ICS subscribe (15 min refresh, per-calendar color/visibility)
- [ ] Google OAuth PKCE + keyring (needs user client ID)
- [ ] Microsoft Graph OAuth PKCE + keyring (needs user client ID)
- [ ] docs/CALENDAR_SETUP.md
- [ ] Meeting-link detect (incl. Webex) + Join opens URL
- [ ] 5-min reminder → toast + compact bar Join
- [ ] All design states: none/events/empty/error+reconnect

### Phase 7 — Capture
- [ ] Snip via `ms-screenclip:` URI
- [ ] Record via windows-capture (ffmpeg fallback) → MP4 in
      `Videos\Float Recordings`; recording compact activity w/ elapsed
- [ ] `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` on our window

### Phase 8 — Polish & release
- [ ] Launch at startup (autostart plugin honoring settings.launch)
- [ ] Hide-in-fullscreen trigger suppression verified
- [ ] Idle CPU <1%, no memory growth
- [ ] `.github/workflows/build.yml` (windows-latest: cargo test + tauri
      build + installer artifact)
- [ ] docs/TEST_CHECKLIST.md manual matrix
- [ ] docs/DECISIONS.md dep justifications (keep updated every phase)
- [x] Privacy rules written into AGENTS.md (PRIVACY), docs/DECISIONS.md and
      docs/PRIVACY.md; code audited against them (2026-11-22)

## Privacy audit (2026-11-22)
- [x] `docs/PRIVACY.md`: principles, data inventory (settings.json, notes,
      logs, webview/localStorage, planned calendar cache + Credential Manager),
      network inventory with every `reqwest`/`fetch(`/`XMLHttpRequest`/
      `WebSocket`/`http(s)://`/plugin/remote font+image finding.
- [x] `src-tauri/tauri.conf.json`: production CSP allows only local assets,
      `data:` images, inline styles and Tauri IPC; `devCsp` adds only the local
      Vite server (`ws://localhost:8443`). No remote fonts/images/scripts/
      frames/objects/workers. `appDirectoriesOverride.log` pins logs to
      `%APPDATA%\Float\logs`.
- [x] `src-tauri/src/lib.rs`: log plugin off unless `FLOAT_DEBUG=1`, file
      target only, 1 MB cap with `KeepOne` rotation, no note bodies logged.
- [x] Violations listed, not removed (owner rule for this session): remote
      Unsplash wallpaper in `src/App.tsx:78`; dormant Google Analytics
      injection in `vite.config.ts:184-205`. Both are blocked by CSP in the
      Tauri webview; proposed removals recorded in docs/PRIVACY.md and
      docs/DECISIONS.md.
- [x] Verification run: `pnpm build` ✓, `cargo test` ✓ (112 passed),
      `pnpm tauri dev` ✓ (Vite ready on :8443, Rust compiled,
      `target\debug\float.exe` launched).
- [x] Pre-existing build blocker repaired so the above could be verified:
      `src/components/NotesView.tsx` was missing the `)}` that closes
      `{listOpen && (` and had one extra `</div>`; both restored to the `HEAD`
      structure. No design, copy or animation content changed.

## Notes tab crash + view isolation (2026-11-22)
- [x] Added `src/components/ErrorBoundary.tsx`: reusable class boundary with a
      small centered panel-styled fallback ("Something went wrong in {Tab}"),
      "Try again" (remounts children via a key) and, for Notes, "Open notes
      folder". Logs only the error message + React component stack at error
      level via `logWebview` → `float.log`; no note text or file contents.
      On catch it restores the 172 px minimum panel height.
- [x] Wrapped **all six** tab views in `Notch.tsx:930-1000` (`home`, `timer`,
      `calendar`, `notes`, `capture`, `settings`) so one broken view cannot
      take down the notch. Notes passes `onHeightChange` and
      `showOpenNotesFolder`.
- [x] Root cause found (not guessed) in `NotesView.tsx` + service wiring:
  - mount effect called `noteService.list().then(...)`; the mock service's
    `list()` is **synchronous**, so `.then` is undefined → immediate crash.
  - render read `listed.length` on the **Promise** returned by the Tauri
    service; later `setRecent(noteService.list())` stored a Promise, so
    `recent.slice(...)` threw on render.
  - service method names differ (`create`/`remove` vs `delete`), `subscribe`
    exists on neither, `save()` returns only `{id, modifiedMs}` and was
    written straight into editor state, `updatedAt` vs `modifiedMs` made
    `timeAgo()` return "NaNd ago", and the two duplicate new-note effects
    created two drafts per request.
- [x] Fixes: normalize every note through `safeNote`/`safeNoteList` (defaults
      for missing/null fields), `loadNotes()` always resolves to `Note[]` and
      swallows load failures, create/remove/subscribe are feature-detected,
      saves update only the id instead of replacing the note, invalid
      timestamps fall back to `Date.now()`, the duplicate new-note effect was
      collapsed to one, and timers/listeners are cleaned up on unmount.
      JSX/design unchanged.
- [x] Rust: error-level logs are now always enabled (debug/info still require
      `FLOAT_DEBUG=1`) so the boundary's crash line reaches `float.log`.
      Privacy docs updated to match.
- [x] Mount tests in `src/components/NotesView.test.ts` (jsdom + Vite SSR
      loader): no notes, sync throw + rejected promise, note with missing
      fields, plus an ErrorBoundary fallback test. `pnpm test` ✓ 34 passing;
      `pnpm exec tsc --noEmit` ✓ 0 errors; `pnpm build` ✓.

## Global shortcuts (Phase 2 follow-up, 2026-11-22)
- [x] Added `src-tauri/src/hotkeys.rs`: pure `Alt+N` / `Alt+C` / `Alt+Z`
      parser (modifier required, canonical duplicate keys), `MOD_NOREPEAT`
      registration, cross-platform status model, defensive duplicate check,
      and parser/status/duplicate unit tests.
- [x] Registered with Win32 `RegisterHotKey` on the existing keyboard-hook
      message thread (`src-tauri/src/triggers.rs`); `WM_HOTKEY` and settings
      reloads are dispatched in that `GetMessageW` loop. Old ids are always
      unregistered before new ones are applied. No new crate/capability:
      justification recorded in `docs/DECISIONS.md`.
- [x] Added persisted settings `hotkeysEnabled` (default on), `hotkeyNotes`
      (`Alt+N`), `hotkeyNext` (`Alt+C`) and `hotkeyPrev` (`Alt+Z`) with serde
      defaults in both `NotchSettings::default()` and `DEFAULT_SETTINGS`;
      partial/old `settings.json` files keep working. `settings_set` wakes the
      hotkey thread so changes apply live.
- [x] `Alt+C` / `Alt+Z` call `MediaState::send_command(Next/Prev)` on the
      displayed session through `spawn_blocking`; no simulated keys, no
      relookup, no second session, no action when no session exists.
- [x] Added gated `TriggerKind::OpenNotes` / `open-notes` to `window_mode.rs`
      (one fresh payload queued until `ui_ready`; watchdog remains Open-only).
      Rust ignores it when `hideInFullscreen` is on and a fullscreen app is in
      front; media shortcuts are unaffected.
- [x] Rust focuses the window on the notes hotkey: `set_focus()` first, then
      AttachThreadInput/BringWindowToTop/SetForegroundWindow/SetFocus fallback,
      logging which method worked. Frontend also makes a best-effort focus
      call after opening.
- [x] Frontend `open-notes`: closed → pinned Notes with a new empty note and
      title focus; open on Notes → flush and close; open on another tab →
      flush, switch to Notes and create a new note. Tab switches and closes
      now flush pending note saves through a registered `NotesView` callback.
- [x] Settings "SHORTCUTS" section: master "Global shortcuts" toggle plus the
      three rows using the existing capture field. Captures require a modifier,
      reject duplicates and double-Alt collisions, and registration failures
      arrive through `hotkeys-status` / `hotkeys_status` (red
      "In use by another app" text).
- [x] `Alt+N`, `Alt+C` and `Alt+Z` cannot count toward double-Alt: unit test
      added to `AltTapTracker`. Frontend shortcut rules covered by
      `src/services/shortcuts.test.ts`.
- [x] Docs updated: `docs/DECISIONS.md` (no-new-dependency justification),
      `docs/PRIVACY.md` (settings keys + local IPC event), `docs/FEATURES.md`
      and this file.
- [x] Verification: `cargo test` ✓ 117 passed, `cargo check` ✓ (existing
      `notes.rs` stub warnings only), `pnpm exec tsc --noEmit` ✓ 0 errors,
      `pnpm test` ✓ 38 passed, `pnpm build` ✓.

## Mic/camera usage indicator (Phase 5 follow-up, 2026-11-23)
- [x] Added `src-tauri/src/privacy_indicator.rs`: read-only HKCU
      CapabilityAccessManager polling every 1 s for both `microphone` and
      `webcam`; desktop apps under `NonPackaged` (exe path `#`-escaped) and
      packaged apps by family name; QWORD FILETIME start/stop values only.
      No audio/video API, no admin rights, no network.
- [x] Silent first snapshot; `detect_starts(prev, curr)` fires only on a
      new/changed `LastUsedTimeStart` with `start > stop`. Unit tests cover new
      start, changed start, unchanged, stop, stale baseline, two apps, separate
      mic/camera tracking, friendly names, silent baseline, cooldown and
      combining.
- [x] 30 s cooldown per friendly app/device kind; mic + camera starts within
      1 s are buffered one poll and emitted as one `privacy-state` event
      `{ kinds: ["microphone" | "camera"], app }`.
- [x] Windows registry access uses the existing `windows` crate's new
      `Win32_System_Registry` feature (no new crate); with `FLOAT_DEBUG=1` raw
      subkey names are logged locally. Justification in `docs/DECISIONS.md`.
- [x] Frontend `privacyService.subscribe` uses `registerTauriListener`; new
      starts become 4 s transient activities at priority 97 above charging,
      hover `"none"`, orange mic / green camera (both for combined), fixed
      labels and truncated trailing app name.
- [x] Added persisted `compactPrivacy` (default true in Rust and TypeScript),
      a "Mic & camera" row in COMPACT BAR using the shared Toggle, live apply,
      `compactBar` master respect, and full suppression while the panel is open.
- [x] Docs updated: `docs/DECISIONS.md` (route + no-new-crate justification),
      `docs/PRIVACY.md` (timestamp-only, local-only, nothing leaves the PC),
      `docs/FEATURES.md` and this file.
- [x] Verification: `cargo test` ✓ 129 passed, `cargo check` ✓ (pre-existing
      `notes.rs` warnings only), `pnpm exec tsc --noEmit` ✓ 0 errors,
      `pnpm test` ✓ 42 passed, `pnpm build` ✓.

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

## Last session (Phase 4B - timer UI + real wiring)

The Timer tab now talks to the Rust engine and was rebuilt as a single focus.

- `tauriServices.ts` gained the real `timerService`: `get`/`subscribe` over
  `timer-state` (listener registered through `registerTauriListener` so
  `ui_ready` waits for it) plus `timer_get/set/start/pause/resume/reset/adjust/
  restart`. Snapshots are de-duplicated by `capturedAtMs`. The mock service
  keeps the same interface and state shape with a wall-clock state machine,
  250 ms done flip, and localStorage `notch-timer-duration` /
  `notch-timer-last-custom`.
- `src/live/useTimer.ts` exposes state, `lastCustomMs` and `remainingMs`,
  which is interpolated every 250 ms from `endsAtMs` while running. `Notch.tsx`
  no longer owns a 1 s interval or `timerRemaining`/`timerRunning`/
  `timerComplete`; compact timer/paused/done activities are derived from the
  hook.
- New `TimerView` (panel height 190 in every state, content centred, no card):
  44px tabular time (`M:SS`, `H:MM:SS` from 1 h); idle −/+ hover circles
  (tap 1 min, Shift 5, hold accelerates), wheel and ArrowUp/Down (1 min),
  microwave digit editor with a 2px caret (Enter/Esc/click-away), four 28px
  text chips + last custom, and a 120x36 white Start pill. Running swaps the
  chips for a 220x2 progress line with "Ends 3:42 PM" and shows a 36px reset
  circle + Pause; paused dims to white/60 with "Paused"/Resume; done pulses
  `0:00` with "Time's up"/Restart. Space starts/pauses/resumes while the tab
  is open. Chips/progress cross-fade 160 ms, hover buttons 120 ms, all
  opacity/translate; reduced motion shortens/removes the pulse.
- Start/Restart still collapse into the compact bar after 450 ms, gated on
  `compactTimer`; pause/reset/done cancel a pending collapse.
- Rust payload now also carries `lastCustomMs` for the custom chip, idle
  `timer_adjust` persists the new `timerDurationMs`, and the engine caches the
  last custom duration at startup.

Verification: `cargo test` 96 passing, `cargo check` clean, `tsc --noEmit`
clean, `pnpm build` succeeds, `pnpm test` 30 passing.

## Last session (Phase 4B - timer UI + real wiring)

The Timer tab now talks to the Rust engine and was rebuilt as a single focus.

- `tauriServices.ts` gained the real `timerService`: `get`/`subscribe` over
  `timer-state` (listener registered through `registerTauriListener` so
  `ui_ready` waits for it) plus `timer_get/set/start/pause/resume/reset/adjust/
  restart`. Snapshots are de-duplicated by `capturedAtMs`. The mock service
  keeps the same interface and state shape with a wall-clock state machine,
  250 ms done flip, and localStorage `notch-timer-duration` /
  `notch-timer-last-custom`.
- `src/live/useTimer.ts` exposes state, `lastCustomMs` and `remainingMs`,
  which is interpolated every 250 ms from `endsAtMs` while running. `Notch.tsx`
  no longer owns a 1 s interval or `timerRemaining`/`timerRunning`/
  `timerComplete`; compact timer/paused/done activities are derived from the
  hook.
- New `TimerView` (panel height 190 in every state, content centred, no card):
  44px tabular time (`M:SS`, `H:MM:SS` from 1 h); idle −/+ hover circles
  (tap 1 min, Shift 5, hold accelerates), wheel and ArrowUp/Down (1 min),
  microwave digit editor with a 2px caret (Enter/Esc/click-away), four 28px
  text chips + last custom, and a 120×36 white Start pill. Running swaps the
  chips for a 220×2 progress line with "Ends 3:42 PM" and shows a 36px reset
  circle + Pause; paused dims to white/60 with "Paused"/Resume; done pulses
  `0:00` with "Time's up"/Restart. Space starts/pauses/resumes while the tab
  is open. Chips/progress cross-fade 160 ms, hover buttons 120 ms, all
  opacity/translate; reduced motion shortens/removes the pulse.
- Start/Restart still collapse into the compact bar after 450 ms, gated on
  `compactTimer`; pause/reset/done cancel a pending collapse.
- Rust payload now also carries `lastCustomMs` for the custom chip, idle
  `timer_adjust` persists the new `timerDurationMs`, and the engine caches the
  last custom duration at startup.

Verification: `cargo test` 96 passing, `cargo check` clean, `tsc --noEmit`
clean, `pnpm build` succeeds, `pnpm test` 30 passing.

## Last session (Phase 4A - real timer engine)

Rust now owns the focus timer; the frontend keeps its UI unchanged and will be
wired to these commands/events next phase.

`src-tauri/src/timer.rs` has a pure state machine (`imp`, no Tauri/Win32 types)
with Unix-millisecond wall-clock times from `SystemTime`:
`Idle{duration_ms}`, `Running{ends_at_ms,total_ms}`,
`Paused{remaining_ms,total_ms}`, `Done{total_ms}`. Times keep counting through
sleep and fire on wake; a backwards clock jump cannot extend a running timer
(remaining is clamped to the original total and the deadline never moves).
Operations: `set_duration` (idle only), `start`, `pause`, `resume`, `reset`,
`adjust(delta)` (idle duration / running deadline+total / paused remaining+total,
all clamped to 1 s..23:59:59) and `restart` (done only, same total). `tick`
returns `Some(Done)` exactly once.

Tauri layer: a shared `TimerEngine` behind an `Arc<Mutex<..>>`; a 250 ms thread
ticks it and on completion emits `timer-done {totalMs}`, emits the fresh
`timer-state`, plays three `MessageBeep(MB_ICONASTERISK)` 600 ms apart when
`timerSound` is on, and shows a Windows toast "Timer done" with the formatted
duration via `tauri-plugin-notification`. Toast/beep failures are logged and
never break the timer. Commands: `timer_get`, `timer_set`, `timer_start`,
`timer_pause`, `timer_resume`, `timer_reset`, `timer_adjust`,
`timer_restart`; each mutating command returns the new snapshot and emits
`timer-state {state, durationMs, totalMs, remainingMs, capturedAtMs,
endsAtMs (running only)}`. No per-second events: the frontend counts down from
`endsAtMs`.

Persistence (settings.json): `timerDurationMs` (default 25 min), optional
`timerLastCustomMs` (written when a timer starts on a non-preset duration other
than 5/10/25 min) and `timerSound` (`#[serde(default = "default_true")]`). A
running timer is deliberately **not** saved across app restarts; only the
configured duration and preferences survive.

Verification: `cargo test` 96 passing (10 new fake-clock tests: exact-once
done, pause/resume rebasing, adjust in all three mutable states, sub-second
clamping, max-duration clamp, backwards-clock behaviour, reset/restart, preset
rule, toast formatting), `cargo check` clean, `pnpm build` succeeds.

## Last session (startup handshake, logical-open fix, open watchdog)

Reported bug: hover / double-Alt were detected by Rust (log lines present) but
the notch did not appear until a power event happened.

Root cause: `uiService.setUiMode` sent `{open: plan.open}` while the Rust
command parameter is `logical_open` (JS camelCase `logicalOpen`), so
`logical_open` was always `None` and Rust fell back to `mode == "open"`. During
a collapse Rust therefore believed the panel was still open. In addition,
nothing gated triggers on page readiness, so events emitted while the webview
was still loading its listeners were lost (field log: `open-request` at
10:52:56 and 12:07:36 produced no following `set_ui_mode`, while later ones
worked).

Fixes:
- `set_ui_mode` argument name corrected to `logicalOpen` via the pure
  `uiModeArgs(plan)` helper (`windowTransition.ts`), tested.
- Startup handshake: the frontend registers every Tauri listener through
  `services/listenerRegistry.ts`; once all are settled it calls the new
  `ui_ready` command. Rust applies Idle/Stable/logical_open=false, resets the
  Alt tracker, marks itself ready (the tray tooltip reads "Float (starting...)"
  until this point, then "Float"), then delivers queued triggers (at most one
  per kind, newest wins). Each queued entry carries its `Instant`; `ui_ready`
  drops anything older than 1500 ms ("dropped stale trigger (age X ms)") so a
  startup hover is not replayed long after the gesture. Hover and double-Alt
  stay invisible while not ready. All
  trigger emission (hover, double-Alt, Esc, outside click, tray) goes through
  `window_mode::emit_trigger`.
- First plan after `ui_ready` is force-sent with the planKey dedupe bypassed.
- 400 ms open watchdog (`PLAN_SEQ` generation token): if no `set_ui_mode`
  arrives after an emitted `open-request`, Rust logs a warning, applies the
  open geometry (visible, interactive, payload `pinned` preserved), and
  re-emits the event once. Any plan clears the pending watchdog.
- Instrumentation behind `FLOAT_DEBUG=1`: startup mode/phase/logical_open/
  visibility, every trigger decision and emit/queue reason, every received
  `set_ui_mode` (applied or deduped with visible/size/ignore-cursor), plus webview
  side listener registration, request receipt, setUiMode send/dedupe, morph
  start/end `document.visibilityState` and rAF probes, all stamped with
  `performance.now()` and routed to float.log.

Evidence (real app, `FLOAT_DEBUG=1`, cursor placed in the hot zone at process
start): hover dwell fired at 12:32:09 with `ui_ready=false` and was *queued*;
`ui_ready` at 12:32:42 applied idle, reset the tracker and flushed the queued
trigger; the webview received `open-request` at t=33303 ms and Rust applied
visible 700x265 within ~18 ms. All startup JS (module start t=33231 ms,
listeners registered t=33297 ms, handshake complete t=33357 ms) ran while the
native window was still hidden, and `document.visibilityState` reported
`visible` the whole time, with rAF running (+14..19 ms) - so suspect B
(WebView2 suspending a hidden page) does not explain this failure. The old
failure was C (event emitted before the listener existed) compounded by the dev
webview taking ~33 s to load Vite modules.

Verification: `cargo test` (85 passing), `cargo check`, `pnpm build`,
`pnpm test` (30 passing), `pnpm exec tsc --noEmit`.

## Last session (Phase 3D-5B — unified notch/bar transition)
Root cause: the frontend changed `uiMode` and Rust resized the window to the
*target* geometry immediately, while Motion was still animating the container
from the *source* geometry. `AnimatePresence mode="wait"` then unmounted the old
content before mounting the new content, leaving a blank clipped frame. Resize
observers also fired repeatedly during the morph, and the old idle branch sent
a 108x26 fallback that did not match the rendered 120x4 sliver.

The panel and compact bar are now two overlapping layers inside one
`motion.div`. `AnimatePresence` crossfades them (blur + subtle scale) instead of
waiting, and the container uses the same 380/30/0.9 spring for width, height and
corner radius in both directions. The panel layer clips itself so the compact
bar's glow can still overflow the outer container.

A pure planner (`src/live/windowTransition.ts`) plus
`src/live/useWindowTransition.ts` now own the explicit morph. Rust receives one
start plan and one final plan: expanding grows the window before content moves;
collapsing holds the source geometry until the spring settles; the window is
click-through during the morph so no large transparent area captures clicks.
Reversed gestures retarget through the geometry the container was heading
toward, so no intermediate idle sliver or early resize appears. `set_ui_mode`
gained a backward-compatible `phase` argument and deduplicates applied geometry,
so repeated bar-width reports do not cause repeated native resizes.

Follow-up review found two issues and fixed them: the compact bar layer was
left-aligned in the growing container (it slid sideways during a morph) and is
now centered; Rust's `panel_open`/`PANEL_CLOSED` were derived from the held
physical mode, so a double-Alt inside the ~320 ms collapse window was treated as
another close. `set_ui_mode` now takes an optional `logical_open` flag from the
target surface; triggers use that while geometry/visibility still use `mode`.

Verification: `pnpm build`, `pnpm test` (16 passing), `pnpm exec tsc --noEmit`,
`cargo check`, and `cargo test` (81 passing) all succeed. No activity priorities
or business rules changed.

## Last session (power indicator latency + percent-pop fixes)

Two reported bugs: the charging bar appeared 2–4 s late after plug/unplug, and
the bar popped up on every 1% battery drain.

Rust (`src-tauri/src/power.rs`): `POLL_INTERVAL` is now 500 ms (was 2 s). A
changed AC state is emitted on the same poll that sees it; the old "hold 1 s
before emitting" gate is gone. After an emitted AC change an 800 ms flicker
guard holds further AC flips, and when it expires the latest reading wins with
one corrected event. Percent-only changes still emit `transition: "none"`
immediately and never touch the guard. The startup reading is still a silent
baseline; charging-bit-only changes, no-battery, and unknown-AC readings still
emit nothing. Tests were rewritten for the new timing, plus tests for immediate
plug/unplug, a flicker that ends in the final state, and percent-only emit.

Frontend: the decision moved to the pure `reducePowerEvent(state, visible)` in
`src/live/powerActivity.ts` — `plugged`/`unplugged` → `register`, `none` +
visible → `update`, `none` + hidden → `ignore`. This stops a percent drain from
creating the slot. Labels derive only from AC state and percent (never the
BatteryFlag charging bit): plugged → "Charging", or "Charged" at ≥100%;
unplugged → "Battery". An unknown percent shows the label with no trailing
number. With `FLOAT_DEBUG=1` (Rust also sets `window.FLOAT_DEBUG`),
timestamps/deltas are logged at: AC change detected, event emitted, frontend
received, activity registered, bar visible — and the last stage flags >150 ms.

Verification: `cargo test` (82 passing), `cargo check`, `pnpm build`, `pnpm test`
and `pnpm exec tsc --noEmit` all pass.

## Last session (Phase 3D-4 — charging indicator)
`src-tauri/src/power.rs` polls `GetSystemPowerStatus` on a dedicated 2 s thread
using the `Win32_System_Power` windows-crate feature (no new dependency). The
Win32 read is behind `#[cfg(windows)]`; a no-op `start` keeps non-Windows builds
compiling. All decisions go through the pure `decide(state, raw, now_ms)`, so
the 11 unit tests run on any platform. The first usable reading becomes an
un-emitted baseline; a change to ACLineStatus or battery percent must hold for
1 s before `power-state` is emitted. Charging-bit-only changes do not emit,
no-battery/unknown-AC readings emit nothing and reset the baseline, and percent
255 becomes `null`. The payload is `{pluggedIn, charging, percent,
transition}` with transition `plugged`/`unplugged`/`none`.

Frontend: `powerService` subscribes to the Tauri event in the service layer;
mock mode is a no-op. A transition registers a transient `charging` activity
(priority 95, TTL 4000 ms, `hover: "none"`): green bolt + "Charging"/"Charged"
when plugged, battery icon + "Battery" when unplugged (red at ≤20%), trailing
percentage text (`—` when unknown). Percent-only events call `update()` rather
than `register()`, so they refresh the visible number without restarting the
TTL. Turning `compactBar` or `compactCharging` off removes the slot immediately.
`LiveLeading.icon` gained an optional colour and `LiveTrailing` gained a text
variant to support this without a custom renderer.

Verification: `cargo test` (80 passing), `cargo check`, `pnpm build`, and
`pnpm exec tsc --noEmit` all pass. `DECISIONS.md` records the feature/justification.

## Last session (Phase 3D-3 — one compact live bar)
The compact bar is now the only collapsed live surface. `LiveActivity` has
`mode` (persistent/transient), `ttlMs`, `hover` (expand/actions/none),
`actions`, `expandTab`, `maxWidth` and an optional `render` override. The
picker prefers any live transient over every persistent activity, then highest
priority, then most-recent registration; the store removes expired transients,
which falls back to the best persistent activity or retracts to the sliver.
`PickActivity` unit tests cover all four rules plus expiry fallback.

Registered slots: recording (persistent/100, pulsing `#ff453a` dot, `MM:SS`
or `H:MM:SS`, hover-only stop, otherwise opens Capture), timer
(persistent/60, 14px ring, `M:SS`, paused dim + pause icon, hover expands
Timer), timer-done (transient/90/5000 ms, preserved pulse), meeting
(transient/80/6000 ms, "{title} in 5 min" + compact Join, 900 ms delay and
same-event-never-re-shows), and music (persistent/10, unchanged 3 s paused
hold). The bar is content-fit 120–260 wide (timer caps itself at 170) instead
of a fixed 150 minimum; `CollapsedPill.tsx` and all `livePill` branches are
gone. Hover only expands for `hover: "expand"`; click opens the activity's
tab pinned. Timer Start and Capture Record dissolve into the bar after 450 ms
when their child setting is on; manual close/pause/preset cancels that beat.

Settings now have a "COMPACT BAR" section: master `compactBar` plus indented
`compactMusic`, `compactTimer`, `compactRecording` and `compactCharging`. They
apply live and all default to true in both TypeScript and Rust
(`#[serde(default = "default_true")]`). `FEATURES.md`, `LAYOUT.md`,
`ARCHITECTURE.md` and `DECISIONS.md` were updated to drop the old pill wording.

Verification: `pnpm build`, `pnpm test` (7 passing), `pnpm exec tsc --noEmit`,
`cargo check`, and `cargo test` (69 passing) all succeed. Cargo prints only an
environment warning about garbage-collecting an old incremental directory.

## Last session (dedicated music-app compact bar)
`media-state` now carries `isMusicApp` and `albumTitle`. `isMusicApp` is true
for session class 0 (Spotify, Apple Music/iTunes, YouTube Music, Amazon Music,
Tidal, Deezer); a browser qualifies only when SMTC reports a non-empty album,
controlled by `BROWSER_ALBUM_HEURISTIC`. With `FLOAT_DEBUG=1` every browser
decision logs its album title and result. The compact bar registers only for
those sessions while they play; the 3 s paused hold and the full music card
are unchanged. Pure classifier tests cover Spotify, Apple Music, Edge with and
without an album, and VLC.

## Last session (album art type sniffing)
SMTC `ContentType()` is not trusted for album art any more. The thumbnail's
leading bytes decide the format: PNG `89 50 4E 47`, JPEG `FF D8 FF`, GIF
`GIF8`, BMP `BM`, WebP `RIFF…WEBP`. Native PNG/JPEG bytes are embedded with
their sniffed MIME (oversized art still becomes a 256 px JPEG); GIF/BMP/WebP
are always re-encoded to a 256 px JPEG; unrecognised bytes are decoded and
re-encoded as PNG, or dropped when undecodable. The data URL is exactly
`data:<mime>;base64,<data>`; with `FLOAT_DEBUG=1` its first 40 characters and
total length are logged. The image crate now enables its `gif`/`bmp`/`webp`
decoders for this (justified in DECISIONS.md).

Frontend: the music card's album-art `<img>` now has an `onError` handler that
logs the MIME and data-URL length through the existing `plugin:log|log` command
(the `log:default` capability was added) and shows the gradient/AudioLines
placeholder. A new art URL clears the failure. Pure helpers (`sniff_art_format`,
`reencode_png`, `art_data_url`) are unit-tested.

## Last session (Phase 3D — compact live bar)
Phase 3D adds a reusable activity system in `src/live`: typed activity slots,
a priority picker, an external register/update/remove store, and one
`CompactBar` that renders any activity. Music was the first registered slot
(recording/timer/reminder moved onto the same bar in Phase 3D-3). The music bar appears
while the shown session plays, stays for 3 s with flat bars after a pause, and
returns whenever the panel closes. Leading is 20x20 art (or the app initial on
the track gradient); the label is the title; the trailing 3-bar equalizer and a
28% glow use the art's dominant colour (fallback `#f09ab7`).

The bar is 30 high and 120-260 wide, bottom radius 15, flush to the top edge.
It springs from the 120x4 sliver (380/30/0.9), content fades from 6 px blur,
titles crossfade with a 5 px slide (260 ms), and reduced motion uses 150 ms
fades. Hover opens Home unpinned, click opens pinned, and fullscreen apps hide
the bar when `hideInFullscreen` is on. Rust pill mode now takes the real bar
size: `(barWidth + 32) x (barHeight + 18)` logical, centered, input accepted;
idle stays hidden and click-through. `compactBar` defaults true in Rust and
TypeScript and is exposed as the master "Show compact bar" toggle in Settings.

## Last session (Phase 3C part 2 — late album art, skip icons, transport-only real card)
Art is now cached by title + artist + album. A missing cover is never final:
while the chosen session is Playing, the poll thread retries once per second for
15 s after each track change, re-calling `TryGetMediaPropertiesAsync` because
Apple Music may only expose `Thumbnail()` in a later fetch; retries stop as soon
as art arrives. Images over 400 KB are decoded with `image` (png/jpeg only),
fitted into 256 px and re-encoded as JPEG quality 85 before base64; if decoding
fails the original bytes are used when under 3 MB, otherwise art stays null.
`FLOAT_DEBUG=1` logs every attempt: `Thumbnail()` present, stream opened, byte
length, content type, whether downscaling ran, and the final result.

The music card uses lucide `SkipBack`/`SkipForward` (17 px, `fill-current`,
strokeWidth 1.2) for previous/next; `Rewind` is gone. Real mode keeps only the
centered previous/play-pause/next row — lyrics, volume, the `...` menu and
Output device remain mock-only, and no Rust volume code was added.

## Last session (Phase 3C part 1 — session priority, clock, single-target controls)
Session choice is now a pure `choose_candidate()` over `SessionInfo` +
`SessionHistory` (unit-tested): each AUMID is classified (case-insensitive
substring) as music (Spotify, Apple Music/iTunes, YouTube Music, Amazon Music,
Tidal, Deezer), other, or browser (Chrome, Edge, Brave, Firefox, Opera,
Vivaldi). Every session's `playing_since` and `last_playing_at` are kept per key
(AUMID, plus `#index` when two sessions share one). Among playing sessions the
lowest class wins, then the earliest `playing_since`; a tie prefers
`GetCurrentSession()`, then `GetSessions()` order. With nothing playing the most
recent `last_playing_at` wins, then the current session, then none. Switches log
the key, app ID, class and reason; `FLOAT_DEBUG=1` also logs each session ID
once so unknown apps (e.g. a YouTube Music PWA reporting a browser AUMID) can
be added to the lists.

Clock: elapsed and remaining are whole floored seconds, so remaining is
`floor(duration) - floor(position)` and the two always add up. `capturedAtMs` is
taken right after the timeline read, before art. With `FLOAT_DEBUG=1` the shown
session logs raw position, `LastUpdatedTime`, rate, corrected position and
`capturedAtMs` every 5 s.

Controls: the poll thread stores the chosen
`GlobalSystemMediaTransportControlsSession` object itself; commands call
`TryTogglePlayPauseAsync` / `TrySkipNextAsync` / `TrySkipPreviousAsync` /
`TryChangePlaybackPositionAsync` on that object and never re-find a session by
AUMID or fall back to `GetCurrentSession()`, so exactly one app is affected.
Every command logs the app ID and its bool result. No media-key simulation.

## Last session (Phase 3B — accurate position, stable session, working controls)
Position: `TimelineProperties.Position()` is only the value at
`LastUpdatedTime()`, so it now advances by `(now - lastUpdated) * PlaybackRate`
while `Playing` (rate defaults to 1.0 when null/error). Zero or future
`LastUpdatedTime` and negative elapsed time are ignored. The result is clamped
to the track duration when one exists, `playbackRate` is part of the payload,
and the UI interpolates with it. Unit tests cover paused, +15 s, rate 2.0,
zero/future timestamps and end clamping.

Resync: on each poll the UI-projected position (last emitted `positionMs` +
elapsed × rate, clamped) is compared with the corrected one; more than 1500 ms
difference emits a fresh `media-state`. A switch of chosen session also emits
immediately.

Session choice is now stable: (a) keep the shown session while it is still
`Playing`; (b) `GetCurrentSession()` if playing; (c) first playing session;
(d) the paused current session; else none. Every switch logs the AUMID, so two
playing apps no longer flicker.

Controls: a std mpsc channel carries `Toggle` / `Next` / `Prev` / `Seek(ms)` to
the `float-media` thread, which resolves the shown session by AUMID and calls
`TryTogglePlayPauseAsync`, `TrySkipNextAsync`, `TrySkipPreviousAsync` or
`TryChangePlaybackPositionAsync(ms * 10_000)`. Tauri commands
(`media_toggle/next/prev/seek`) wrap the wait in `spawn_blocking` with a 2 s
timeout, so the async runtime never blocks on WinRT. A command switches polling
to 150 ms for 1.5 s, then back to 500 ms; false/error results are logged.

Frontend: play/pause updates immediately then follows the next `media-state`;
previous/next call the commands; the scrubber (only when duration is known)
moves the bar while dragging and sends `media_seek` on pointer/key release. The
dropped position is used optimistically for interpolation until the next event.

## Last session (Phase 3A — real now playing)
Done: `src-tauri/src/music.rs` polls Windows SMTC on one dedicated
`float-media` thread (`RoInitialize(RO_INIT_MULTITHREADED)`, 500 ms). It picks
the first `Playing` session from `GetSessions()`, else `GetCurrentSession()`.
Per session it reads `TryGetMediaPropertiesAsync` (title/artist),
`SourceAppUserModelId`, `GetPlaybackInfo().PlaybackStatus()` and
`GetTimelineProperties` (100 ns → ms; duration null when end ≤ start). The
windows 0.62 blocking wait is `IAsyncOperation::join()` (there is no `.get()`),
and it never runs off the media thread. Thumbnail bytes go through `DataReader`
→ local base64 → `data:<mime>;base64,…`, cached by title+artist and skipped
above 1.5 MB. AUMIDs map to Spotify/Apple Music/Chrome/Edge/Brave/Firefox/Opera,
else the readable AUMID/exe name.

`media_snapshot` returns the cached payload; `media-state` fires only when
title, artist, source, playing or duration changes, plus one `null` when the
session disappears. Position ticks never emit; the UI interpolates from
`positionMs` + `capturedAtMs`. `FLOAT_DEBUG=1` logs the chosen AUMID and every
state change. Errors log once (manager/session separately) and polling
continues.

Frontend: `tauriServices.mediaService` listens first, then calls
`media_snapshot`, maps payload → `Track` (`artDataUrl`, `playing`,
`positionMs`, `durationMs`, `capturedAtMs`), dedupes stale snapshots by
`capturedAtMs`, and returns `null` for no session. `Notch` shows the idle home
for `null`/before the first snapshot, interpolates position every 100 ms while
playing, and never falls back to mock songs in the Tauri app. `MusicCard` shows
the art (gradient fallback), samples an 8×8 canvas average for the glow, hides
the progress row when `durationMs` is null, and hides the not-yet-wired
transport/volume controls in real mode.

Half-done / follow-ups (superseded by Phase 3B):
- Transport (play/pause/skip/seek) and seek resync landed in Phase 3B; system
  volume was dropped from the UI in Phase 3C.2.
- Base64 is hand-written to avoid a one-use dependency.

## Last session (Rust-owned panel open state + three close paths)
Done: `window_mode::AppState` (managed with `app.manage`) now holds
`panel_open`, `panel_pinned` and the panel height + physical screen rect
(480 logical wide, centered on the monitor, height = the reported
`panel_height`, top at the monitor's y). `set_ui_mode` gained a `pinned` arg and
updates all of it: `open` = true, `idle`/`pill` = false. When the open state
flips, `triggers::reset_alt_tracker` clears the double-Alt tracker and logs it.
Double-Alt: the `WH_KEYBOARD_LL` hook reads `panel_open` and emits
`close-request {source:"alt"}` or `open-request {source:"alt", pinned:true}`;
the old `toggle-request` path is gone. Escape (`0x1B`) on an open panel emits
`close-request {source:"esc"}`. A new `WH_MOUSE_LL` thread + message loop emits
`close-request {source:"outside"}` on L/R/M down outside the stored panel rect,
but only when the panel is open *and* pinned (hover opens stay unpinned). No key
or click is ever swallowed. `AltTapTracker::reset()` has two unit tests.
Logging: `double alt: panel_open=…, emitting …`, every `close-request` with its
source, every tracker reset.
Frontend: `close-request` runs the existing `close()` (which also clears the
pin); `open-request` honours `pinned` from Rust (hover false, Alt/tray true) and
focuses on keyboard open (best effort — Esc does not need focus). The
`toggle-request` listener/decision was removed. `AppState` panel rect has tests
at 100/125/150% and offset monitors.
Half-done / follow-ups:
- Close decisions keep `panel_open` true until the frontend reports `idle`
  (~250 ms after the close animation), so a rapid second double-Alt during the
  animation closes again instead of reopening.
- Outside-click close is pinned-only by design; the settings view opened from
  the tray is pinned, hover settings is not.
- Alt-menu suppression (inject unassigned VK) is still not implemented.

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
- Triggers: Rust WH_KEYBOARD_LL double-Alt hook, JS sequence Tauri-gated, pure tracker unit-tested.
- Triggers: physical-px `in_hot_zone` + new `DwellTracker`, closed-only `open-request {source:"hover"}`, unpinned hover open.
- Settings: single-value `Toggle` (white/black on, #303030/#9a9a9a off), four toggles default true, serde defaults for partial settings.json.
- Settings: `settingsVersion` 2 migration, live `RwLock` state for both triggers, suppression-reason logs, redesigned toggle (white/#000 on, #2a2a2a/#8a8a8a off, dimmed label, focus ring), optimistic save + revert.
- Panel state: `AppState` open/pinned/rect, double-Alt decides open/close in Rust, Esc + outside-click `close-request`, Alt tracker reset on open/close.
- Music 3A: `music.rs` SMTC poll thread (MTA, 500 ms), cached `media_snapshot`, state-only `media-state`, base64 art + AUMID source labels; real mediaService + interpolated progress + art glow; no fake songs in Tauri mode.
- Music 3B: position corrected from `LastUpdatedTime` + `PlaybackRate` (seek resync >1.5 s), stable sticky session choice, `media_toggle/next/prev/seek` over an mpsc channel with 2 s timeout + 150 ms fast polling; frontend transport/scrubber wired.
- Music 3C.1: pure class/priority chooser (music > other > browser, earliest playing, current-session tie-break, paused fallback) with history keyed by AUMID (+index), floored clock labels, 5 s `FLOAT_DEBUG` diagnostics, and commands sent only to the stored displayed session object.
- Music 3C.2: album art keyed by title+artist+album with 1 s/15 s late-arrival retries and `image` downscaling (≤256 px JPEG q85 over 400 KB), `SkipBack`/`SkipForward` icons, real card limited to centered transport (volume/lyrics/menu dropped from the UI).
- Live 3D: reusable `src/live` activity slots + priority picker/store, music compact bar (30 px, 150–260 wide, art/initial leading, dominant-colour equalizer + 28% glow, 3 s paused hold), hover/click/close behavior, fullscreen suppression, `compactBar` setting, and frontend-sized pill geometry `(barWidth + 32) x (barHeight + 18)` with Node unit tests for `pickActivity`.
- Album art: magic-byte format sniffing replaces `ContentType()`, non-PNG/JPEG art is re-encoded to a 256 px JPEG (unknown → PNG or null), exact `data:<mime>;base64,<data>` URL plus `FLOAT_DEBUG` head/length log, frontend `<img onError>` logs via `plugin:log|log` and shows the placeholder.
- Startup: `ui_ready` handshake after all listeners register (pre-ready triggers queued, one per kind), `set_ui_mode` now sends `logicalOpen` via `uiModeArgs`, first post-ready plan force-sent past planKey dedupe, 400 ms open watchdog forcing visible geometry plus one re-emit, and `FLOAT_DEBUG` instrumentation on Rust trigger/plan paths and webview listener/morph/rAF timings.
- Phase 4A: pure wall-clock timer state machine (idle/running/paused/done), `timer_*` commands + `timer-state`/`timer-done` events, 250 ms tick thread, toast + MessageBeep alert, settings persistence (`timerDurationMs`/`timerLastCustomMs`/`timerSound`), tray tooltip unaffected; 10 fake-clock tests.
- Phase 4B: real/mock `timerService`, `useTimer()` local 250 ms countdown, new single-focus TimerView (190 px), Space toggle, compact activities derived from the Rust snapshot, idle adjust persistence + `lastCustomMs` in the payload.
