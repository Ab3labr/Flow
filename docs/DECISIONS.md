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

## Dependency justifications

Backend
- image 0.25 (default-features = false, features ["png", "jpeg", "gif", "bmp",
  "webp"]): decode album art over 400 KB and re-encode it as a 256 px JPEG
  before base64, so the `media-state` event stays small; the extra decoders let
  GIF/BMP/WebP art be sniffed by magic bytes and normalised to JPEG (unknown
  bytes fall back to PNG).

Frontend
- @fontsource-variable/inter: bundle Inter locally (no remote font import,
  CSP-safe, works offline)
- Front-end unit tests use Node's built-in `node:test` + type stripping
  (`pnpm test`); no extra test-runner dependency.

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

Phase 8
- tauri-plugin-autostart: launch at startup
