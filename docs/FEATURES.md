# FEATURES — Float notch panel

Description of the approved prototype. Do not change visuals, copy, timings
or spring values unless broken or a real feature forces it.

## App shell (prototype only)

`src/App.tsx` renders a fake 1920x1080 desktop stage (scaled to fit), an
Unsplash background, a fake Windows taskbar, and `<Notch />`. In the real
app the desktop is the user's actual screen; this chrome is removed once
the Tauri shell works.

## Panel open/close (Notch.tsx)

State: `expanded`, `pinned`, `tab`, timers (`hoverTimer`, `leaveTimer`,
`dissolveTimer`), plus the shared live-activity store with per-transient TTL
timers. Timer state comes from `useTimer()` (see TimerView).

### Open triggers
- **Hot zone:** `openFromEdge()` — pointer entering the top edge strip
  (`topEdge` setting, only when closed and no live bar) starts a
  **180 ms** timer, then `setExpanded(true)`.
- **Double-Alt:** two Alt presses within **350 ms**, no other key between,
  toggles. Requires `settings.altAlt` and `settings.shortcut === "Alt Alt"`.
  Any Alt keydown/keyup is `preventDefault()`ed (browser menu-bar
  suppression). `lastAlt` resets on any non-Alt key.
- **Custom shortcut:** `settings.shortcut` (e.g. `Ctrl + Alt + F`) matched
  from modifiers + key name (`Ctrl/Alt/Shift/Meta`, `Space`, single chars
  uppercased, `Control`→`Ctrl`). Opens **pinned**; closes if expanded.
- **Compact bar click:** expands the panel pinned on the activity's `expandTab`.

### Close paths
- **Hover close:** pointer leave of strip/panel, not pinned → **400 ms**
  then collapse.
- **Pin:** click on panel background (not button/input/textarea) sets
  `pinned`.
- **Esc:** collapse + unpin.
- **Outside pointerdown:** `close()` (also clears pin). In Tauri the
  webview won't see outside clicks — Rust must emit close on blur.

### Global hotkeys (real app)
- Defaults: `Alt+N` opens Notes, `Alt+C` next track, `Alt+Z` previous track.
  All three are rebindable in Settings, with a master `Global shortcuts`
  toggle; changes register live. A binding needs at least one modifier and
  cannot duplicate another global binding or the double-Alt shortcut.
- `Alt+N` is gated like other open triggers: with `hideInFullscreen` on, it is
  ignored while a fullscreen app is in front. It tries to focus the notch; if
  the webview still lacks keyboard focus, the Rust fallback attaches to the
  foreground thread and repeats the focus switch. The method used is logged.
- Pressing `Alt+N` on a closed panel opens it pinned on Notes with a **new
  empty note** and the title focused. On the Notes tab it flushes any pending
  save and closes. On any other tab it flushes, switches to Notes and opens a
  new note.
- `Alt+C` / `Alt+Z` call the same displayed-session transport commands as the
  music-card buttons. They never simulate media keys, never choose a second
  session and do nothing when no media session exists.
- Held combinations fire once (`MOD_NOREPEAT`). `Alt+N`, `Alt+C` and `Alt+Z`
  never count toward the double-Alt trigger.
- If Windows cannot register a combination, the app stays open and the row
  shows `In use by another app` in red in Settings.

### Mic/camera usage indicator (real app)
- Reads only Windows' own HKCU CapabilityAccessManager usage timestamps
  (`LastUsedTimeStart` / `LastUsedTimeStop`) once a second. No audio or video
  device is ever opened and no data leaves the PC.
- The startup snapshot is a silent baseline. A later start fires only when
  `LastUsedTimeStart` is new/changed and `start > stop` (`stop = 0` still in
  use); stopping alone never fires. The same app and device kind has a 30 s
  cooldown, and mic + camera starts within 1 s become one event.
- Each start shows a 4 s transient compact-bar activity (priority 97, above
  charging; hover `"none"`). Leading: orange (`#ff9f0a`) mic, green
  (`#30d158`) camera, or both for a combined start. Label: `"Microphone"`,
  `"Camera"` or `"Camera and mic"`. Trailing: the friendly app name, truncated.
- Nothing is shown while the full panel is open. `compactPrivacy` defaults on,
  follows the `compactBar` master toggle, and has a `"Mic & camera"` row in the
  COMPACT BAR settings section; changes apply live.

## Notes (Phase 5B)

### Card
- Card `rounded-[14px] bg-[#121212]`, `min-h-[120px] max-h-[340px]`, spring
  height (380/30/0.9); content = textarea scrollHeight + 68 clamped
  120–340; panel height = card + 52 clamped 172–340.
- Title 14px semibold (placeholder "Start typing…"); textarea 13px
  leading-1.5 white/75, min-h-42px, auto-grow.
- Footer `h-6 border-t white/5`: "Saved" check (a small button; see
  Session below); "Saving..." if a save takes longer than 400 ms; "Couldn't
  save" in red (#ff453a) if it fails (keeps text in editor and retries on next
  change); "Too large" in red when a paste would exceed 2 MB.
- Autosave: **600 ms** after last keystroke. A pending save must be flushed
  **IMMEDIATELY** (not debounced) when: the user switches tab; the panel
  closes or collapses (Esc, outside click, hover-close, double-Alt); the
  window is hidden or loses focus; the user presses New note or opens another
  note; and on "flush-request" from Rust (then emit "flush-done" from frontend).
- Never create files for empty drafts; two quick edits produce one queued save.
- Serializes save: never run two saves for the same note at once.
- A paste goes through the same autosave path as typing. A ~500 KB paste is
  accepted without a layout loop; over **2 MB** the paste is rejected with
  "Too large" and is not saved.

### Session and reopen rule (2026-11-24)
- The current note (id, title, body, dirty flag, caret, scroll, save queue)
  lives in a session store in `Notch`, not in `NotesView`. Closing the panel
  or changing tabs unmounts only the editor; typed or pasted text never
  disappears.
- Reopening Notes shows the same note, scroll position and caret at the end,
  unless:
  - the last edit was more than `notesNewAfterMinutes` ago (default **720**,
    `0` = never), in which case a fresh blank draft opens and the old note
    stays saved in the recent list; or
  - the user pressed **New note**.
- `chooseNoteOnOpen(nowMs, lastEditedMs, lastId, fileExists, newAfterMinutes,
  newNotePressed)` is the pure rule; it also returns a fresh draft when the
  remembered file no longer exists.
- Across restarts `notesLastId` and `notesLastEditedMs` persist in
  `settings.json`; a missing file opens a fresh draft.
- Alt+N and the Notes tab open the **current** note via the rule above. They
  never create a note. Only the idle-home "New note" chip and the footer "+"
  button create a note. A fresh blank note stays a draft with no file until it
  has content.

### Recent list popover (hover-over-grid)
- Top 5 from `list(5)`, with the title ("Untitled note" if empty) and time ago.
- Clicking opens that note (flush the current one first).
- Hover shows a trash icon that calls `remove()` (Recycle Bin).
- If the open note is deleted, its text stays as a new draft so nothing typed
  is lost.
- Footer link "Open notes folder" calls `notes_open_folder`.

### New note
- "New note" creates an in-memory draft with no file. The first save with
  non-empty content creates the file (Rust returns the id); use that id for
  later saves.
- The idle-home "New note" chip and the Notes footer "+" both call the same
  session `startNewNote` path and focus the title. Alt+N never creates one.

### Folder path
- The Settings "Notes folder" row shows the real full path from
  `notes_folder_path` (middle ellipsis if long, full path in the tooltip).
- "Open folder" calls `notes_open_folder` (create the folder if missing, then
  Explorer). The recent popover has the same action.

### Footer indicator
- "Saved" is a small button after a successful save. Its tooltip is the file
  name; clicking calls `notes_reveal(id)` to open Explorer with that file
  selected. Drafts with no file are disabled.
- "Saving..." only if a save takes longer than 400 ms.
- "Couldn't save" in red (#ff453a) if it fails (log the error; keeps the text
  in the editor and retries on the next change).
- "Too large" in red when the note would exceed 2 MB; it is not saved.

### Autosave
- **600 ms** after last keystroke.
- A pending save must be flushed **IMMEDIATELY** (not debounced) when:
  - the user switches tab
  - the panel closes or collapses (Esc, outside click, hover-close, double-Alt)
  - the window is hidden or loses focus
  - the user presses New note or opens another note
  - on "flush-request" from Rust (then emit "flush-done" from frontend)
- Never create files for empty drafts
- Two quick edits produce one queued save
- Serializes saves: never run two saves for the same note at once
- A paste uses the same autosave path. ~500 KB is accepted; over 2 MB is
  rejected with "Too large" and not saved.

## Panel open/close (Notch.tsx)

State: `expanded`, `pinned`, `tab`, timers (`hoverTimer`, `leaveTimer`,
`dissolveTimer`), plus the shared live-activity store with per-transient TTL
timers. Timer state comes from `useTimer()` (see TimerView).

### Open triggers
- **Hot zone:** `openFromEdge()` — pointer entering the top edge strip
  (`topEdge` setting, only when closed and no live bar) starts a
  **180 ms** timer, then `setExpanded(true)`.
- **Double-Alt:** two Alt presses within **350 ms**, no other key between,
  toggles. Requires `settings.altAlt` and `settings.shortcut === "Alt Alt"`.
  Any Alt keydown/keyup is `preventDefault()`ed (browser menu-bar
  suppression). `lastAlt` resets on any non-Alt key.
- **Custom shortcut:** `settings.shortcut` (e.g. `Ctrl + Alt + F`) matched
  from modifiers + key name (`Ctrl/Alt/Shift/Meta`, `Space`, single chars
  uppercased, `Control`→`Ctrl`). Opens **pinned**; closes if expanded.
- **Compact bar click:** expands the panel pinned on the activity's `expandTab`.

### Close paths
- **Hover close:** pointer leave of strip/panel, not pinned → **400 ms**
  then collapse.
- **Pin:** click on panel background (not button/input/textarea) sets
  `pinned`.
- **Esc:** collapse + unpin.
- **Outside pointerdown:** `close()` (also clears pin). In Tauri the
  webview won't see outside clicks — Rust must emit close on blur.

## Other tabs (Timer, Calendar, Capture, Settings)

Content per tab as designed, unchanged.

### TimerView
Single focus, centred, no card background. Panel height stays **190** in every
timer state. Smallest text is 11px.

- Time: 44px semibold, tabular numerals, `-.04em`, white; `M:SS`, or
  `H:MM:SS` from one hour. While running the display counts down locally every
  **250 ms** from the Rust `endsAtMs` (no per-second events).
- Idle hover: 28px ghost −/+ circles fade in 120 ms on either side of the
  time, 20px away. Tap = 1 min, Shift = 5 min; press-and-hold repeats with the
  existing acceleration (420 ms first delay → 60 ms floor). Wheel over the
  time = 1 min; ArrowUp/ArrowDown = 1 min while the time is focused.
- Digit editor (idle): click the time and type microwave-style — the last two
  digits are seconds, so `1,5,0,0` shows `15:00`. A 2px white caret follows
  the value; **Enter** sets it, **Esc** or a click away cancels. Clamped to
  1 s–23:59:59.
- Idle chips: one row of up to four 28px text-only chips, gap 6 — `5m`,
  `10m`, `25m` and the last custom duration if it is not a preset (whole
  minutes as `7m`, otherwise `1:30`). Selected: `#1c1c1c` background, white
  text; the rest white/45.
- Primary pill: 36px high, **120** wide, 12px semibold black on white — Start.
- Running: −/+ now adjust the remaining time (1/Shift 5). The chips row
  cross-fades (160 ms) into a 2px, 220px progress line (track white/15, white
  fill, rounded, remaining/total) with `Ends 3:42 PM` below (11px white/40).
  Buttons: 36px `#1c1c1c` reset circle + white Pause pill.
- Paused: time at white/60, caption "Paused", primary Resume, reset visible.
- Done: `0:00` with a soft opacity pulse, caption "Time's up", primary
  Restart, reset returns to idle.
- Motion: chips ↔ progress and the primary label cross-fade (160 ms), hover
  buttons 120 ms; opacity/translate only, no blur. Presses use the shared
  `whileTap` 0.92 animation.
- Space starts/pauses/resumes while the Timer tab is open. Start/Restart
  dissolve the panel into the compact bar after **450 ms** only when
  `compactTimer` is on. The compact bar keeps the timer ring + `M:SS`
  (`H:MM:SS` past an hour) from the Rust snapshot; completion raises the
  transient `timer-done` activity (5 s).

## CalendarView
- `h-[178px]`.
- No accounts: centered `rounded-[14px] bg-[#121212]` card — "Connect your
  calendar" + sub + white "Connect calendar" pill.
- Header `h-8`: "WED, 4 NOV" 11px bold .04em `#ff453a` (**hardcoded — real
  impl shows actual date**) + "Today" 13px semibold; right "＋ Add" pill.
- Sync error (first erroring account): `h-8 rounded-lg bg-[#121212]
  px-2.5` — "Couldn't sync {Provider}. Reconnect" `11px #ff453a` + Reconnect
  button; list shrinks to `h-[96px]`.
- Empty: `h-[126px] rounded-[14px] bg-[#121212]` "No events today".
- Events: `h-[138px]` scroll list (`notch-scroll pr-1`), rows `h-[42px]
  rounded-xl border-[#1f1f1f] bg-[#121212] px-2.5`: 3px `h-6 w-[3px]` color
  bar, time `w-[52px]` 11px tabular white/40, title 11px medium + detail
  11px white/40, JoinButton if meeting link.
- Connect sheet: overlay `inset-0 z-30 rounded-[14px] border-[#1f1f1f]
  bg-[#121212] p-3`; steps providers → signin → connecting (0.8 s spinner)
  → connected (check + "{n} accounts available" + Done). Mock fakes 850 ms.
- Providers: google "G", outlook "M", icloud "A", ics "I".

## CaptureView
- `h-[133px]` gap-2.5, two `h-[100px] flex-1 rounded-[14px]
  border-[#1f1f1f] bg-[#121212]` buttons.
- **Snip:** Scissors + "Snip" — **inert today**; real: `ms-screenclip:`.
- **Record:** idle red ring (`h-6 w-6 border-[5px] border-[#ff453a]`) +
  "Record"; recording: pulsing glow (`h-14 w-14 bg-[#ff453a]/15 blur-xl`,
  .35→.85→.35, .8→1.25→.8, 1.7 s) + filled Square + `MM:SS` tabular.
  Mock: 1 s interval counter in Notch state; Record dissolves into the
  compact bar after 450 ms when `compactRecording` is on.

## SettingsView
- Scroll `h-[288px]` (`notch-scroll`).
- "NOTCH SETTINGS" label; toggle card `rounded-[14px] border-[#1f1f1f]
  bg-[#121212]`, rows `h-8 px-2.5 border-b white/6`:
  "Open at top edge" (`topEdge`), "Open with Alt Alt" (`altAlt` + shortcut
  input `w-[58px]` readOnly capturing keydown), "Hide in fullscreen apps"
  (`hideInFullscreen`), "Launch at startup" (`launchAtStartup`), "Demo: upcoming event"
  (`upcomingEvent`), "Demo: no calendars connected" (`noCalendars`).
- Toggle: `h-5 w-9` (36x20, knob 14), one `value` drives track, knob
  position/colour and label; on = white track + black knob at x 19 + full-white
  label, off = `#2a2a2a` track + 1 px `#444444` inset outline + `#8a8a8a` knob
  at x 3 + `white/50` label; spring 500/32, `whileTap 0.94`, brighter track on
  hover, `role="switch"`, `aria-checked`, focus-visible ring, Space/Enter.
- "SHORTCUTS" section: master `hotkeysEnabled` ("Global shortcuts") plus
  `hotkeyNotes`, `hotkeyNext`, `hotkeyPrev` rows ("Open Notes", "Next track",
  "Previous track"). Each uses the same readOnly shortcut-capture field as
  "Open with Alt Alt"; click it and press a modifier + key. Invalid, duplicate
  or double-Alt-colliding captures are rejected in place. Rust registration
  failures (for example `In use by another app`) appear in red under the row;
  the master's children dim when it is off. All changes apply live.
- "COMPACT BAR" section: master `compactBar` ("Show compact bar") plus
  indented children `compactMusic`, `compactTimer`, `compactRecording`,
  `compactCharging`, `compactPrivacy` (Music/Timer/Recording/Charging/
  Mic & camera). Children dim when the master is off; all toggles apply live.

## Meeting links
`getMeetingProvider` scans `location + description` for
`meet.google.com`→Google Meet, `zoom.us`→Zoom,
`teams.microsoft.com`→Teams (mark letters G/Z/T). **Webex missing — spec
requires it.** JoinButton: `#30d158` pill `h-6 px-2 text-[11px]
font-semibold text-black`, optional halo when live (opacity .12→.3→.12,
scale 1→1.08, 3.8 s). Join is inert — real: open URL. `live` = mock flag;
real: `startsAt <= now < endsAt`.

## Animation constants (do not regress)
- Surface changes: spring 380/30/0.9 for width/height/radius in both
  directions; opacity fades in .22. Rust holds the source window on collapse
  and grows before expansion, then applies one final stable size.
- Content enter delay .13 dur .24 ease [.22,1,.36,1]; exit .12.
- Tab switch .22 same ease. Tab pill spring 420/32. Press .92 spring
  600/30. Toggle knob 500/32.
- `prefers-reduced-motion` → 150 ms everywhere.

## Prototype-only / known gaps
See docs/GAP_ANALYSIS.md for the full fake→real mapping. Highlights:
lyrics/Output-device/Join/Snip/Open-folder inert; "Demo:" toggles;
hardcoded "WED, 4 NOV"; events list doesn't refresh
with time; stale `accountCount` in connect sheet; no error boundary;
`Track.source` is a 3-name union; `CalendarEvent.detail` preformatted;
services synchronous mock-only; fake desktop in App.tsx; Google Fonts +
remote image need CSP thought; no tests; directory is not a git repo yet;
`.mise.toml` pins node 22 + pnpm 10.34.3; `vite.config.ts` has Figma Make
plugins (dev/build only); index.html has figma comments + empty title;
package name `figma-make-app`.