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
| Complete → 3 s pill only | Rust toast notification + sound, `timer-done` event → pill (Rust ensures window visible for pill) |
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
| Meeting detect: Meet/Zoom/Teams only | Add Webex; extract first matching URL → `meetingUrl` |
| Join inert | `open` crate / opener plugin → default browser |
| Reminder: JS effect finds event ≤5 min, 900 ms delay, 6 s pill | Rust scheduler (~30 s tick): event with link starting ≤5 min → toast + `meeting-reminder` event → pill w/ Join |
| "Demo:" toggles | Remove or keep only in mock mode (decide Phase 6) |
| Events don't refresh with time | Periodic refresh + on-open refresh + `calendar-changed` events |
| `accountCount` stale in sheet | Pass live count or compute at connect time |
| iCloud provider listed | Spec skips Apple — keep UI row but mark unsupported? Decide: spec lists providers in UI already; implement google/outlook/ics; iCloud row can show "not supported" or be removed — flag to user |
| Local calendar | NEW: default offline store, add/edit/delete via "+" button (needs small event editor UI — check design; "+" currently opens connect sheet! Spec says "+" adds events — design tension, resolve in Phase 6) |

## Capture
| Prototype | Real |
|---|---|
| Snip button inert | Phase 7a: `ms-screenclip:` URI; later custom overlay → PNG to `Pictures\Float Screenshots` + clipboard + toast |
| Record = local bool + 1 s counter | Windows Graphics Capture (`windows-capture` crate) → H.264 MP4 in `Videos\Float Recordings`; ffmpeg sidecar fallback; start on primary monitor |
| Elapsed in Notch state | Rust owns recording; `recording-state` events → pill + Capture tab |
| Stop from Capture tab only | Also stop from pill (pill click expands; add stop affordance? spec: "stop from the pill or the Capture tab" — pill needs a stop control; may require small UI addition — flag) |
| Float window captured | `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` |
| No audio | Stretch goal, skip |

## Settings
| Prototype | Real |
|---|---|
| `useState` defaults, lost on reload | `settings_get`/`settings_set` → `%APPDATA%\Float\settings.json`, atomic write |
| Shortcut input captures keydown string | Same UI; Rust hook parses "Ctrl + Alt + X" format |
| "Launch at startup" toggle inert | Persist now; `tauri-plugin-autostart` Phase 8 |
| "Hide in fullscreen apps" inert | Rust foreground-window fullscreen detection suppresses triggers |
| "Notes folder" static text + inert button | Show configured path; Open folder → Explorer; folder picker? (spec: changeable in Settings — needs picker or text input; flag UX) |
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

## Open questions for user (flagged, not blocking)
1. iCloud provider row: implement, hide, or mark unsupported? (Spec doesn't
   cover it; phases list Google/Microsoft/ICS only.)
2. "+" button in CalendarView currently opens the connect sheet; spec says
   it adds events. Need a small add-event editor — confirm OK to extend UI.
3. "Demo:" toggles in Settings — keep (mock parity) or remove in real build?
4. Stop-recording control on the live pill — pill is 108x26 with just
   elapsed text today; spec wants stop from pill. Minimal change: make
   pill click stop? Or add tiny stop button? Confirm.
5. Notes folder picker UX (native folder dialog via tauri dialog plugin —
   adds dependency).
6. Tray "Open Float" pinned or hover-style? (Assume unpinned.)
7. Webex detection: `webex.com` domain match OK?
8. Recurring calendar events (RRULE): expand or show instances only?
   (Spec silent — assume basic RRULE expansion for daily/weekly or
   document limitation.)
