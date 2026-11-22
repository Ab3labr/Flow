# Float

Windows desktop "notch" utility: a hidden-by-default panel anchored
top-center that opens on double-tap Alt or a top-edge hot zone. Music card,
calendar card, and tabs for Timer, Calendar, Notes, Capture, Settings.

**Stack:** React 19 + TypeScript + Tailwind CSS v4 + Motion frontend;
Tauri 2 shell + Rust backend (`src-tauri/`).

## Docs (read these)

- `docs/FEATURES.md` — every feature/state/animation, as designed
- `docs/LAYOUT.md` — dimensions, colors, type
- `docs/ARCHITECTURE.md` — structure, service layer, Tauri plan
- `docs/GAP_ANALYSIS.md` — fake→real mapping per feature
- `docs/PROGRESS.md` — phase checklist + status (keep updated)
- `docs/DECISIONS.md` — dependency justifications

## Run

```
pnpm install
pnpm dev                 # browser prototype with fake data (VITE_USE_MOCK=1)
pnpm tauri dev           # real app (builds Rust + vite)
pnpm tauri build         # release installer
cd src-tauri && cargo test
```

## Rules

- Do NOT change visuals, proportions, colors, copy or animations unless
  broken or a real feature requires it. The design is approved.
- All data access stays behind the service layer
  (`src/services/*Service`): mock impl (`VITE_USE_MOCK=1`) + real impl
  calling Tauri commands/events.
- Windows-specific Rust behind `#[cfg(windows)]` with trait abstractions
  and mock fallbacks — must compile/test on non-Windows.
- Commit after every phase with a clear message.
- No new dependencies without a one-line justification in
  `docs/DECISIONS.md`.
- No features beyond docs/PROGRESS.md phases (exceptions: tray icon,
  single instance, settings persistence).
- When blocked: record it in docs/PROGRESS.md, use the documented
  fallback, keep going.

## PRIVACY (permanent owner decision)

Float is free, easy to use and private forever.

- No server or database of its own, no account, no analytics, no telemetry,
  no crash reporting and no automatic update check.
- Everything the user creates (notes, settings, timer and calendar data)
  lives only on their PC.
- Network access exists only for features the user turns on themselves and
  goes straight to the service they chose (their own Google, Microsoft or
  ICS calendar). No Float-owned server in between. Nothing is uploaded
  without a user action.
- Sending song titles, event titles, note text or any other personal content
  to a third party is NOT allowed — including online album-art lookups.
- The webview CSP must block all external `connect`/`font`/`img`/`script`/
  `frame` sources; only Tauri IPC (and the localhost dev server in dev) are
  permitted. See `src-tauri/tauri.conf.json` and `docs/PRIVACY.md`.
- Debug/info logging is off unless `FLOAT_DEBUG=1`; error-level crash logs are
  always recorded. Logs stay in `%APPDATA%\Float\logs`, rotate/truncate at 1 MB,
  never include note bodies or file contents, and are never uploaded.
- Keep `docs/PRIVACY.md` current: every new file/store or outbound request
  must be added to its inventories before the change lands.

## Project structure

- `src/App.tsx` — entry component (prototype fake desktop; real app
  renders only the notch)
- `src/components/` — Notch, TopBar, MusicCard, EventCard, TimerView,
  CalendarView, CalendarConnect, NotesView, CaptureView, SettingsView,
  CollapsedPill
- `src/services/` — types.ts, index.ts, mockServices.ts, tauriServices.ts
  (see ARCHITECTURE.md)
- `src-tauri/` — Rust backend (created in Phase 1)
- `.mise.toml` — node 22, pnpm 10.34.3
- `vite.config.ts` — Vite + Tailwind v4 + Figma Make plugins; verify it
  doesn't break `pnpm tauri dev`

Prototype details: see AGENTS history / docs/FEATURES.md. Figma Make dev
server notes no longer apply — use `pnpm tauri dev`.
