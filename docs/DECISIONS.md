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
- Recording's compact-bar slot gets a small hover-only stop button; clicking
  the rest of the bar still opens Capture.
- Notes folder picker deferred to Phase 8. Webex matches webex.com. Recurrence: server-side for Google/Outlook, RRULE crate for ICS.
- Charging/unplugging shows a 4 s transient compact-bar activity (priority 95,
  no hover); percent-only updates refresh it without restarting its lifetime.
  A percent-only drain never opens the bar: with nothing visible it is ignored.
  The label comes from the AC state and percent, never the BatteryFlag charging
  bit; an unknown percent shows the label alone.
- The notch and compact bar are one window/container. During a morph the window
  is click-through and keeps the source geometry (collapse) or grows first
  (expand); Rust gets one start plan and one final stable plan, not per-frame
  resizes.
- `set_ui_mode` carries an explicit `logical_open` flag (JS camelCase
  `logicalOpen`, built by `uiModeArgs`): triggers act on the target surface, so
  a collapsing panel is already logically closed while its geometry is held.
- Startup handshake: all frontend Tauri listeners register through
  `listenerRegistry`, then `ui_ready` lets Rust reset to idle/stable/
  logical_open=false and flush triggers queued before readiness (at most one
  per kind). The first plan after `ui_ready` bypasses the planKey dedupe.
- Open-request watchdog: no `set_ui_mode` within 400 ms means log a warning,
  apply the visible open geometry (payload `pinned` preserved), re-emit once;
  any received plan clears the pending watchdog.

Global shortcuts
- Defaults are `Alt+N` (open Notes), `Alt+C` (next track) and `Alt+Z`
  (previous track). All three plus a master `hotkeysEnabled` switch are
  rebindable in Settings and apply live; the master defaults to on.
- Win32 `RegisterHotKey` with `MOD_NOREPEAT` on the existing keyboard-hook
  message thread, not `tauri-plugin-global-shortcut`. Justification: `windows`
  0.62 is already a dependency with `Win32_UI_Input_KeyboardAndMouse` and
  `Win32_UI_WindowsAndMessaging` enabled, and the thread already runs a
  `GetMessageW` loop. The plugin would add a crate plus Tauri capability for
  the same OS API. No new dependency or permission is needed.
- Media shortcuts call `MediaState::send_command(Next/Prev)` on the one
  displayed session, the same object the music-card buttons use. No simulated
  media keys, no app-ID relookup and no second session is skipped.
- A failed `RegisterHotKey` is logged at error level, stored in memory and
  emitted as `hotkeys-status` for the Settings tab; the app keeps running.
  Startup state is also available through the `hotkeys_status` command for
  listeners that mounted after the initial emit.

Mic/camera indicator
- Read only the HKCU CapabilityAccessManager `microphone` / `webcam` usage
  timestamps. Float never opens an audio/video device, never captures media,
  and never asks for admin rights. The snapshot lives only in process memory
  and the only payload is the local `privacy-state` Tauri event.
- Use the existing `windows` crate with its `Win32_System_Registry` feature for
  read-only `RegOpenKeyExW` / `RegEnumKeyExW` / `RegQueryValueExW`. Adding
  `winreg` would duplicate an already-present dependency for the same Win32
  API. Justification recorded below under Dependency justifications.
- Startup is a silent baseline; later polls fire only on a new/changed
  `LastUsedTimeStart` with `start > stop`. Keep a 30 s per-app/per-kind
  cooldown and join mic+camera starts within 1 s into one event.
- `compactPrivacy` defaults on, follows the `compactBar` master toggle, and is
  suppressed while the full panel is open.

## Privacy decisions (permanent owner decision)

- Free, easy to use and private forever. No Float-owned server or database,
  no account, no analytics, no telemetry, no crash reporting, no automatic
  update check. User data (notes, settings, timer, calendar) stays on the PC.
- Network access only for features the user explicitly turns on, directly to
  the chosen service (Google / Microsoft / ICS). No Float proxy in between,
  nothing uploaded without a user action. No third-party calls that carry
  song titles, event titles, note text or other personal content; in
  particular online album-art lookups are forbidden. SMTC thumbnail art is
  read from the local Windows session only. The mic/camera indicator reads only
  local HKCU usage timestamps (which app started using a device and when);
  no app content, audio, video or device list is read, and no outbound request
  is added. Nothing leaves the PC.
- Webview CSP is allow-list only: Tauri IPC plus the local Vite dev server
  in `devCsp`; no remote fonts, images, scripts, frames or fetches in the
  production `csp`. Inter is bundled locally (`@fontsource-variable/inter`).
- Logs are diagnostic only: debug/info off unless `FLOAT_DEBUG=1`, error-level
  crash logs always recorded, written solely to `%APPDATA%\Float\logs`, capped
  at 1 MB with one-file rotation, never contain note bodies or file contents,
  never uploaded. The log directory is pinned with
  `app > appDirectoriesOverride.log`. Error level is always on so a view crash
  can be diagnosed without requiring the user to set an environment variable.
- The remote Unsplash wallpaper in the prototype `App.tsx` and the dormant
  Figma-build Google Analytics injection in `vite.config.ts` conflict with
  these rules. They are listed in `docs/PRIVACY.md` for removal in a later
  session; this session records the rules and blocks them at the CSP layer
  without changing feature behavior.

## Dependency justifications

Backend
- image 0.25 (default-features = false, features ["png", "jpeg", "gif", "bmp",
  "webp"]): decode album art over 400 KB and re-encode it as a 256 px JPEG
  before base64, so the `media-state` event stays small; the extra decoders let
  GIF/BMP/WebP art be sniffed by magic bytes and normalised to JPEG (unknown
  bytes fall back to PNG).
- windows `Win32_System_Registry` feature (no new crate): read-only
  `RegOpenKeyExW` / `RegEnumKeyExW` / `RegQueryValueExW` over the HKCU
  CapabilityAccessManager usage records for the mic/camera indicator. `windows`
  0.62 is already required for the shell, SMTC, power and hotkeys; `winreg`
  would add a second registry wrapper for the same OS calls. No admin rights,
  no device access, no network.

Frontend
- @fontsource-variable/inter: bundle Inter locally (no remote font import,
  CSP-safe, works offline)
- Front-end unit tests use Node's built-in `node:test` + type stripping
  (`pnpm test`); no extra test-runner dependency.
- `jsdom` ^31 + `@types/jsdom` ^30 (dev only): mount React components in the
  existing `node:test` runner. TSX and `import.meta.env` are supplied by Vite's
  own SSR loader (already a dependency), so no vitest/testing-library/tsx is
  needed. Required by the NotesView crash regression tests.

Phase 1
- @tauri-apps/api ^2.12.5: Tauri 2 frontend bindings
- @tauri-apps/cli ^2.12.5: Tauri 2 CLI for dev/build
- windows ^0.62 (Win32_Foundation, Win32_System_Threading): single-instance enforcement via mutex (spec requirement)
- tauri `tray-icon` feature (pulls the tray-icon crate): system tray icon and
  menu, per owner decision (Open Float / Settings / Quit)

Phase 3D-4
- windows `Win32_System_Power` feature (no new crate): `GetSystemPowerStatus`
  is the Win32 API for AC/battery state; 500 ms poll, immediate emit on change,
  800 ms flicker guard, and all decision logic stays pure and cross-platform
  testable.

Phase 4A
- tauri-plugin-notification ^2: a native Windows toast is the only surface that
  reaches the user while the notch is hidden; it is Tauri's official
  notification wrapper (one capability entry, no hand-written WinRT). The
  alert sound uses the already-required `windows` crate: `MessageBeep` lives in
  the `Win32_System_Diagnostics_Debug` feature (plus the existing
  `Win32_UI_WindowsAndMessaging` for `MB_ICONASTERISK`), so there is no extra
  runtime dependency.

Phase 5
- trash ^5.4.0: cross-platform Recycle Bin support for `notes_delete`. The
  `trash` crate abstracts over platform-specific APIs (Windows: SHFileOperationW with
  FOF_ALLOWUNDO, macOS: trash-put, freedesktop: xdg-desktop-portal). It also
  provides a coinit_apartmentthreaded feature for Windows OLE initialization. Using
  this crate avoids implementing platform-specific recycle bin logic ourselves and
  ensures notes are sent to the Recycle Bin rather than permanently deleted.
- settingsFolder configurable per-phase decision: Documents\Float Notes default,
  configurable via Settings; do NOT build from `%USERPROFILE%`.
- `windows` `Win32_System_SystemInformation` feature (no new crate): `GetLocalTime`
  is the Win32 API used to stamp note file names with the user's local time;
  a pure UTC fallback keeps non-Windows builds compiling and testable.

Phase 8
- tauri-plugin-autostart: launch at startup
