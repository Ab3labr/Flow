# GAP ANALYSIS — fake → real

For each feature: what the prototype fakes today, and what the real
implementation needs.

## Panel shell & triggers
| Prototype | Real |
|---|---|
| Fake 1920x1080 desktop + taskbar in App.tsx | Delete; transparent Tauri window over real desktop |
| Hot zone = 1px-tall `h-1 w-60` div + 180 ms JS timer, only while webview hovered | Rust 60 Hz cursor poll, 240x4 px zone, 180 ms dwell, mouse-button + fullscreen suppression, multi-monitor/DPI |
| Double-Alt + custom shortcut in window keydown (only works when webview focused) | WH_KEYBOARD_LL global hook, 350 ms double-Alt state machine, shortcut parser, Alt-menu suppression (F24 inject) |
| Outside-click close via window pointerdown | Impossible in Tauri (webview sees no outside clicks) → Rust window-blur → close event |
| Esc close via window keydown | Works when focused; also add Rust-side Esc in hook for robustness |
| Hover-close 400 ms / pin / pill timers in JS | Keep in frontend (webview alive); window visibility driven by `ui-mode` events |
| Panel always "exists" in DOM | Rust shows/hides or resizes window per state; `set_ignore_cursor_events` when closed |

## Music
| Prototype | Real |
|---|---|
| `mediaService.getTracks()` static array; `trackIndex` cycles it | SMTC `GlobalSystemMediaTransportControlsSessionManager`: pick current session else most recent |
| `playing` local bool; progress ticks 0.1 s/100 ms; wraps to next track | `PlaybackInfo` playback status + `TimelineProperties` (position/duration/rate); interpolate position in UI from last update; seek via `TryChangePlaybackPosition` |
| Prev/next = local index | `TrySkipPrevious/TrySkipNext` on the session (no playlist) |
| Play/pause = local toggle | `TryTogglePlayPause` |
| Volume slider = local state | `IAudioEndpointVolume` master scalar + mute; subscribe `OnNotify` |
| `Track.colors` hardcoded triple; gradient art | Decode `MediaProperties.Thumbnail` stream → PNG data URL; sample dominant colors (downscale average) for glow/art gradient; `hasArt=false` → placeholder |
| `source` union Spotify/YouTube/Apple Music | Map `SourceAppUserModelId` → friendly name (Spotify, Apple Music, Chrome, Edge, etc.) |
| duration always number | `duration: number \| null` — hide progress bar when null (live/streams) |
| No-session → MusicCard still shows fake track | `nowPlaying = null` → IdleHome (when no upcoming event) |
| Mock "next track" wrap | N/A — real transport only |

## Timer
| Prototype | Real |
|---|---|
| `timerRemaining`/`timerRunning`/`timerComplete` in Notch state, 1 s interval | Rust authoritative: `end_instant`, commands set/pause/resume/add, `timer-state` events; survives hidden webview |
| Complete → 5 s `timer-done` compact activity | Rust toast notification + sound, `timer-done` event → transient compact activity (Rust ensures the window is visible) |
| Presets/custom/+1 min local state | Same UI; state via commands; custom times persist in settings.json |
| Ring progress local | From service `remaining/total` |

## Notes
| Prototype | Real |
|---|---|
| localStorage `notch-notes` JSON | `%USERPROFILE%\Documents\Float Notes\*.md`; first line = title; filename slug+timestamp |
| `save` overwrites whole blob | Debounced (600 ms) atomic write temp+rename |
| `list` all notes | Recent 5 by mtime |
| `remove` deletes | Recycle Bin (`trash` crate / IFileOperation) |
| No external-change handling | `notify` watcher → `notes-changed` event → refresh list |
| "Documents\Notch Notes" label (mock) | Folder configurable in Settings; default `Documents\Float Notes` (spec) — update label text |
| "Open folder" inert | `open`/`explorer` the configured folder |
| Plain textarea (no markdown render) | Keep as-is (spec: markdown FILES, editor stays plain) |

## Calendar
| Prototype | Real |
|---|---|
| `calendarAccounts` mock (2 accounts, synced) | settings.json account metadata + per-provider stores; statuses synced/error/connecting |
| `getToday` filters+formats mock events | Aggregator over local file + ICS cache + Google + Graph events; raw `startsAt/endsAt` |
| Connect sheet fakes 850 ms | Real OAuth flows (PKCE + loopback) / ICS URL add / local default |
| `createAccount` instant | Async connect commands; error → `status:"error"` + Reconnect |
| Hardcoded "WED, 4 NOV" | Actual date formatted `ddd, D MMM` |
| `detail` preformatted "in 5 min · 30 min" | Service returns raw fields; format in UI/service consistently |
| `live` flag on one mock event | `startsAt <= now < endsAt` |
| Meeting detect: Meet/Zoom/Teams only | Add Webex (webex.com and subdomains); extract first matching URL → `meetingUrl` |
| Join inert | `open` crate / opener plugin → default browser |
| Reminder: JS effect finds event ≤5 min, 900 ms delay, 6 s meeting activity | Rust scheduler (~30 s tick): event with link starting ≤5 min → toast + `meeting-reminder` event → transient compact activity w/ Join |
| "Demo:" toggles | REMOVED from real build; kept in mock mode only (per owner decision). |
| Events don't refresh with time | Periodic refresh + on-open refresh + `calendar-changed` events |
| `accountCount` stale in sheet | Pass live count or compute at connect time |
| iCloud provider listed | HIDDEN (per owner decision). Users with iCloud calendars can add them via public ICS link. |
| Local calendar | NEW: default offline store, add/edit/delete via "+" button. Per owner decision: "+" opens a minimal add-event sheet (title, start/end time, optional meeting link) in existing style. Connecting calendars stays in Settings and the empty state. |

## Capture
| Prototype | Real |
|---|---|
| Snip button inert | Phase 7a: `ms-screenclip:` URI; later custom overlay → PNG to `Pictures\Float Screenshots` + clipboard + toast |
| Record = local bool + 1 s counter | Windows Graphics Capture (`windows-capture` crate) → H.264 MP4 in `Videos\Float Recordings`; ffmpeg sidecar fallback; start on primary monitor |
| Elapsed in Notch state | Rust owns recording; `recording-state` events → compact activity + Capture tab |
| Stop from bar hover or Capture tab | Keep the hover-only stop button; clicking the rest of the compact bar opens Capture (per owner decision). |
| Float window captured | `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` |
| No audio | Stretch goal, skip |

## Settings
| Prototype | Real |
|---|---|
| `useState` defaults, lost on reload | `settings_get`/`settings_set` → `%APPDATA%\Float\settings.json`, atomic write |
| Shortcut input captures keydown string | Same UI; Rust hook parses "Ctrl + Alt + X" format |
| "Launch at startup" toggle inert | Persist now; `tauri-plugin-autostart` Phase 8 |
| "Hide in fullscreen apps" inert | Rust foreground-window fullscreen detection suppresses triggers |
| "Notes folder" static text + inert button | Show configured path; Open folder → Explorer. Folder picker DEFERRED to Phase 8 (per owner decision). |
| Calendar accounts section | Real accounts; connect/disconnect/visibility/reconnect wired |

## Cross-cutting
| Prototype | Real |
|---|---|
| Synchronous mock services | Async `invoke` + `listen` event subscriptions |
| `VITE_USE_MOCK` doesn't exist | Add env flag; mock stays default in plain browser |
| No tests | Rust unit tests (triggers) + `cargo test` in CI |
| No persistence | settings.json + notes dir + calendar store |
| `package.json` name figma-make-app | Rename to `float` |
| index.html empty title + figma comments | `<title>Float</title>` |
| Google Fonts remote import | Keep for dev; for packaged app rely on system stack or bundle font (CSP) |
| Unsplash remote image | Removed with fake desktop |
| No error boundary | Add simple boundary? Optional — flag |
| `vite.config.ts` Figma plugins | Verify `pnpm tauri dev`/`build` unaffected |
| StrictMode double-effects | Fine; ensure listeners cleaned up |
| `events` memo stale w.r.t. time | Refresh on interval/open/events |
| `Notch` monolith | Keep structure; inject services via props/context minimally |

## Open questions for user — RESOLVED
All 8 questions resolved via owner decisions (see docs/DECISIONS.md):
1. iCloud row: HIDDEN. Users add iCloud calendars via public ICS link.
2. "+" button: EXTENDS UI to open a minimal add-event sheet (title, start/end time, optional meeting link). Connecting calendars stays in Settings/empty state.
3. "Demo:" toggles: Removed from real build; kept in mock mode only.
4. Recording pill stop: Small stop button appears on hover; clicking rest of pill expands panel.
5. Notes folder picker: DEFERRED to Phase 8. Fixed default folder used now.
6. Tray "Open Float": PINNED (opens panel pinned and focused).
7. Webex detection: Yes, match `webex.com` and its subdomains.
8. Recurring events: Google/Outlook expand server-side; ICS uses RRULE crate. Local events are non-recurring in v1 (documented limit).
