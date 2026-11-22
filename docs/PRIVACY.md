# PRIVACY — Float

Permanent owner decision. This document is the audit record: update it when
adding any file/store or outbound request. See `AGENTS.md` (PRIVACY) and
`docs/DECISIONS.md` (Privacy decisions).

## (a) Principles

- The app is free, easy to use and private forever.
- It never stores user data on a server or database of its own and never
  needs an account.
- It has no analytics, telemetry, crash reporting or automatic update check.
- Everything the user creates — notes, settings, timer and calendar data —
  lives only on their PC.
- Network access exists only for features the user turns on themselves and
  goes straight to the service the user chose (their own Google, Microsoft or
  ICS calendar). No Float-owned server in between. Nothing is uploaded
  without a user action.
- Anything that would send song titles, event titles, note text or other
  personal content to a third party (for example online album-art lookups)
  is NOT allowed.
- Debug logs may contain song/event titles, are off unless `FLOAT_DEBUG=1`.
  Error-level crash logs (view name, error message, React component stack) are
  always recorded. All logs live only in `%APPDATA%\Float\logs`, rotate/truncate
  at 1 MB, never include note bodies, note text or file contents, and are never
  uploaded.

## (b) Data inventory — every file or store the app writes

| Store | Path (Windows) | What it holds | Status |
| --- | --- | --- | --- |
| `settings.json` | `%APPDATA%\com.float.app.windows\settings.json` (Tauri `app_config_dir()` = config dir + bundle identifier `com.float.app.windows`) | `settingsVersion`, notch toggles / shortcut, `hotkeysEnabled`, `hotkeyNotes`, `hotkeyNext`, `hotkeyPrev`, `notesFolder` override, `calendarAccounts` metadata, `timerCustomTimes`, `timerDurationMs`, `timerLastCustomMs`, `timerSound`. No credentials or tokens. | Implemented (`src-tauri/src/lib.rs`, `settings` module). |
| `settings.json.tmp` | Same directory as `settings.json` | Transient atomic-write temp file during `settings_set` / version migration. | Implemented. |
| Notes folder | `settings.notesFolder` if set, otherwise `Documents\Float Notes` | Markdown note files named `YYYY-MM-DD HH-mm-ss {slug}.md`; first line `# title`, remainder = body. Personal note text. | Default/folder path decided; the file-I/O commands in `src-tauri/src/notes.rs` are currently stubs (Phase 5 work still pending). |
| Logs | `%APPDATA%\Float\logs\float.log` (pinned by `app > appDirectoriesOverride.log`; rotation keeps one file) | Error-level view-crash lines always (tab name, error message, React component stack). When `FLOAT_DEBUG=1` also startup/trigger/window geometry, media session/title/artist and art diagnostics. Never note bodies, note text or file contents. | Implemented; debug/info off unless `FLOAT_DEBUG=1`, error level always on. |
| Webview storage | Tauri webview data dir under `%LOCALAPPDATA%\com.float.app.windows\` (localStorage / HTTP cache / IndexedDB) | Only what the webview itself caches; the real app uses Tauri commands, not localStorage. | Runtime-managed by WebView2. |
| Browser-prototype localStorage | `notch-notes`, `float-mock-notch-settings`, `notch-timer-duration`, `notch-timer-last-custom` | Mock notes (titles/bodies), mock settings, mock timer values. **Browser prototype only** (`VITE_USE_MOCK=1`); the real app does not use these keys. | Implemented (`src/services/mockServices.ts`). |
| Calendar cache | Planned local ICS cache (`docs/ARCHITECTURE.md`) | Event data fetched directly from the user's own ICS source. | Not implemented; no file written today. No credentials stored by Float. |
| Credential Manager entries | Windows Credential Manager (planned, Phase 8) | OAuth tokens for the user's own Google/Microsoft accounts, if the user connects a calendar. | Not implemented; no `keyring` dependency and no credential writes today. |
| `dist/` | Project build output | Bundled JS/CSS/fonts (build artifact, not user data). | Build-time only. |

## (c) Network inventory — every outbound call found in the code

Search performed for `reqwest`, `fetch(`, `XMLHttpRequest`, `WebSocket`,
`http://`, `https://`, `ws://`, `wss://`, `url(`, `@import`, `src=`, `href=`,
Tauri updater/notification/opener plugins, and remote fonts/images across
`src/`, `src-tauri/src/`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`,
`src-tauri/capabilities/`, `index.html`, `vite.config.ts`,
`.figma/make/site.json`, `package.json` and `dist/index.html`.

| # | Finding | Sends | To whom | Enabling setting / condition | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | `src/App.tsx:78` — CSS `url('https://images.unsplash.com/photo-...')` for the fake desktop wallpaper | HTTP GET for the JPEG, with query params (`crop`, `cs`, `fit`, `fm`, `q`, `w`) and the machine's IP/user agent | images.unsplash.com (Unsplash CDN) | Nothing: always rendered by the prototype `App.tsx`. Previously permitted in Tauri by `img-src ... https:`. | **Violation** (automatic external request / remote image). Proposed removal: replace with a bundled local wallpaper or a CSS gradient. Not removed this session. Now blocked by CSP in the Tauri webview. |
| 2 | `vite.config.ts:184-205` — `figmaSiteConfiguration()` injects `<script src="https://www.googletagmanager.com/gtag/js?id=...">` plus a `dataLayer`/`gtag('config', ...)` snippet | Page views/events, page URL, IP and user agent | Google Tag Manager / Google Analytics | Only when `analytics.googleAnalyticsId` is set in `.figma/make/site.json`. Currently unset (`site.json` has no analytics key), and `dist/index.html` contains no GA script. | **Dormant violation** (analytics mechanism). Proposed removal: delete the analytics branch entirely. Not removed this session; CSP blocks the external script even if configured. |
| 3 | Tauri dev HMR (Vite dev server) — `ws://localhost:8443` plus `http://localhost:8443` | Source/HMR messages, no user content | Local Vite dev server on the same PC | `pnpm tauri dev` / `devUrl: http://localhost:8443`. Allowed in `devCsp` only, never in the production `csp`. | Local-only, permitted. |
| 4 | Tauri IPC — `ipc://localhost` / `http://ipc.localhost` (`invoke`, events, `plugin:log|log`) | Settings, note text, timer state, track metadata, log lines, `hotkeys-status` (registration failures only) | The app's own Rust process on the same PC | Core Tauri capability (`core:default`), always on. | Local-only, permitted. |
| 5 | `src/services/mockServices.ts:75,85` — `https://meet.google.com/notch-review`, `https://teams.microsoft.com/l/meetup-join/notch` stored as calendar `location` strings | Nothing; these are strings, not fetched. A user click would open them via the OS | Google Meet / Teams, only on explicit user action | Mock data only. No `opener` plugin is installed and no `calendar_join` command exists yet. | Not a network call. If wired later, opening must remain an explicit user action. |
| 6 | `src/components/MusicCard.tsx:206` (`track.artDataUrl`) and `src/live/CompactBar.tsx:50` (`leading.url`) — `<img src=...>` | Nothing external; values are `data:` URLs or locally generated strings | Nobody | Always rendered, but no network fetch. | Permitted. |
| 7 | Album art in Rust (`src-tauri/src/music.rs`, `Thumbnail()` / `DataReader`) | Reads the local Windows SMTC thumbnail stream into memory, re-encodes to a `data:` URL | Nobody online — no online album-art lookup exists | Active when a media session exposes art. | Permitted; any future online lookup would violate the rules. |
| 8 | Fonts — `src/main.tsx:4` imports `@fontsource-variable/inter`; `index.html` / `dist/index.html` have no remote `<link>` or `@import url(...)`; `src/index.css` only has `@import "tailwindcss"` | Nothing external; fonts are bundled into `dist/assets/*.woff2` | Nobody | Always. | Permitted. |
| 9 | Rust dependencies — no `reqwest`, no HTTP client, no `updater` plugin, no `opener` plugin in `Cargo.toml`; `tauri.conf.json` has no `plugins.updater` section | Nothing | Nobody | N/A | Permitted. `tauri-plugin-notification` only raises a local Windows toast; it sends nothing. |

## Enforcement

- `src-tauri/tauri.conf.json`: production `csp` allows only local assets,
  `data:` images, inline styles and Tauri IPC; `devCsp` adds only the
  localhost Vite dev server (HTTP + HMR WebSocket). No remote fonts, images,
  scripts, frames, objects or workers.
- `src-tauri/src/lib.rs`: `tauri_plugin_log` is built with
  `LevelFilter::Off` unless `FLOAT_DEBUG=1`, has only the file target under
  `%APPDATA%\Float\logs`, and rotates at 1 MB with `KeepOne`.
- Never add note bodies to any log or event payload sent outside the process.
