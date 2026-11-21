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

### Compact live bar (single live surface)
The old `108x26` live pill is gone. Music, recording, timer, timer-done,
meeting reminders and charging are all `LiveActivity` slots rendered by the
same bar.
Collapsed states are the `120x4` invisible sliver (opacity 0) or the bar
itself. It appears only when master `compactBar` is on, the panel is closed, an
activity exists, and no fullscreen app is in front while `hideInFullscreen` is
on. Bar: height **30**, content-fit width **120–260** (or the activity's
`maxWidth`), bottom radius **15**, flush to the top edge, `bg-black`, panel
shadow. Leading slot 20x20 radius 6 (art, app initial on the track gradient,
icon (including charging bolt/battery), timer ring, recording dot or pulse);
label 11px medium, one line, ellipsis; trailing equalizer/dot/time/pause/text
in the activity colour (fallback `#f09ab7`). A soft glow sits under the bar
in the same colour (28% max, 8px
blur, 3 s pulse while playing, static when paused).

An activity carries `mode: "persistent" | "transient"`, `ttlMs` for transients,
`priority`, `hover: "expand" | "actions" | "none"`, optional `actions`,
`expandTab`, `maxWidth` and an optional `render` override. Picking rules: any
live transient beats every persistent; within each group the highest priority
wins; ties go to the most recently registered activity. When a transient
expires the store removes it, so the bar falls back to the best persistent
activity or retracts to the sliver.

- **Recording:** persistent, priority **100**. Pulsing red dot `#ff453a`
  (0.55→0→0.55 opacity, scale 1→2, 1.8 s) + elapsed `MM:SS` (`H:MM:SS` after
  an hour), tabular numerals. Hover reveals a small stop button; clicking the
  rest opens the Capture tab pinned.
- **Charging:** transient, priority **95**, **4000 ms**, `hover: "none"`.
  Rust polls `GetSystemPowerStatus` every 500 ms, emits a changed AC state on
  the same poll, then holds further AC flips behind an 800 ms flicker guard;
  the first reading at startup is a silent baseline. Plug → green bolt
  (`#30d158`) and "Charging" ("Charged" at 100% or more); unplug → battery
  icon, "Battery", red (`#ff453a`) at 20% or less. The label never uses the
  BatteryFlag charging bit. Trailing text is the percentage, omitted when it is
  unknown. A transition starts the 4 s slot; a percent-only update refreshes a
  visible slot without restarting its lifetime and is ignored when nothing is
  visible. Controlled live by `compactCharging`.
- **Timer:** persistent, priority **60**. 14px remaining/total ring + `M:SS`;
  paused is dimmed with a pause icon. Hover expands to Timer. `maxWidth` 170.
- **Timer done:** transient, priority **90**, **5000 ms**. Preserved pulse
  (scale 1→1.35→1, opacity .5→1→.5, 1.5 s) + "Timer done".
- **Meeting reminder:** transient, priority **80**, **6000 ms**. "{title} in
  5 min" + compact JoinButton. Event starting in `(0, 5 min]` with a meeting
  link → wait **900 ms** → show → expire after 6000 ms. Same event id never
  re-shows.
- **Music:** persistent, priority **10**, unchanged. It registers only while
  the shown session plays; a pause keeps it for **3000 ms** with flat bars,
  then removes it.

Hover expands only for `hover: "expand"` activities; `actions` reveals the
stop control and `none` does neither. Click opens the panel pinned on
`expandTab`, and closing the panel while any persistent activity is alive
returns to the bar. **Timer Start** and **Capture Record** dissolve the panel
into the bar after **450 ms** when that activity's compact toggle is on; with
the toggle off the panel stays open as before.

Motion: the panel and bar are two overlapping layers of one container. Width,
height and bottom radius use the same 380/30/0.9 spring in both directions, so
the panel morphs directly into the bar (and back) without visiting the 120x4
sliver. Outgoing content fades and scales out while incoming content fades in
from 6px blur; titles crossfade with a 5px slide (260 ms). Reduced motion uses
150 ms fades with no blur/scale.

### Panel geometry
Expanded: width **480**, height per tab — home 172, timer 190, calendar
230, capture 185, settings 340, notes dynamic 172→340. Top-center,
top-anchored, `bg-black`, `overflow-hidden`, shadow
`0 12px 34px rgba(0,0,0,.36)` when open or bar.

## Tabs (TopBar)
Left icons: Home, Timer, Calendar, Notes, Capture; Settings pinned right.
Tab buttons `h-6 w-6` rounded-full, 15px icon strokeWidth 1.8, white/60 →
white. Active tab: `layoutId="active-tab"` pill `bg-[#1c1c1c]`, spring
stiffness 420 damping 32. `PressButton`: `whileTap scale 0.92`, spring
600/30.

Content `mt-1.5`, `AnimatePresence mode="wait"` per tab: enter opacity
0→1, blur 6→0, y 8→0; exit opacity 0 blur 5; exit 0.12 s; transition
0.22 s `ease [0.22,1,0.36,1]`.

### Home
- **Music + upcoming event:** `h-[120px]` row, MusicCard flex-1 + EventCard
  148 px wide, spring 380/30/0.9, blur+width 0→148.
- **Idle** (nothing playing AND no upcoming event): `IdleHome` `h-[120px]
  rounded-[14px] bg-[#121212]` — clock 30px semibold tabular `HH:MM` (1 s
  tick), date 12px white/45, pills "Start timer" / "New note" / "Snip".

### MusicCard
- `rounded-[14px] border-[#1f1f1f] bg-[#121212] p-2.5`.
- Art: 48x48 `rounded-[11px]` gradient from track colors, shadow
  `0 8px 24px rgba(0,0,0,.28)`, AudioLines icon; crossfade 0.26 s.
- Glow: `-left-5 -top-7 h-28 w-40 opacity-[0.12] blur-[60px]` 135°
  gradient, fade 0.5 s. **Real: sample colors from album art.**
- Copy: title 13px semibold, artist 11px white/60, source 11px white/35;
  crossfade y ±5, 0.26 s.
- Equalizer: 3 bars `#f09ab7` w-2px, heights `[3,10-i,5,11,3]`, durations
  [0.88,1.12,1.28], stagger 0.08 s; flat when paused.
- Progress: `mt-1.5` row — time `w-7` 11px tabular white/50, invisible
  range input (step 0.1) over `h-1 bg-white/15` track + white fill; hover
  → `h-1.5` + 10px dot (150 ms); `-M:SS` `w-8` right. Mock advances 0.1 s
  per 100 ms tick, wraps to next track at duration.
- Controls `h-[22px]`: Captions (inert), prev/next (Rewind/SkipForward
  17px fill), play/pause 26px circle with icon morph (scale+rotate).
- "…" menu `bottom-8 right-0 w-40 rounded-xl border-white/10 bg-[#202020]
  p-3`: volume slider (`.volume-range` 3px/10px thumb) + "Output device"
  (inert). **Real: system volume via Core Audio.**

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

### CalendarView
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
- `CalendarAccountRow`: color dot, email 11px white/80, status ("Synced
  just now" white/35 or error #ff453a), eye toggle, refresh, Reconnect
  (error only), Disconnect.

### NotesView
- Card `rounded-[14px] bg-[#121212]`, `min-h-[120px] max-h-[288px]`, spring
  height (380/30/0.9); content = textarea scrollHeight + 68 clamped
  120–288; panel height = card + 52 clamped 172–340.
- Title 14px semibold (placeholder "Start typing…"); textarea 13px
  leading-1.5 white/75, min-h-42px, auto-grow.
- Footer `h-6 border-t white/5`: "Saved" check fade-in; "＋ New note" +
  list button.
- Autosave: **600 ms** debounce → `noteService.save`. Mock: localStorage
  `notch-notes`. Real: markdown files, atomic write.
- Recent list: `bottom-7 right-0 w-56 rounded-xl border-white/10
  bg-[#202020] p-1.5`, top 5, rows `h-10` title ("Untitled note") +
  timeAgo ("now"/"Nm"/"Nh"/"Nd ago"), trash icon on hover. Mock deletes;
  real: Recycle Bin.
- `newNoteRequest` → create + focus title (30 ms).

### CaptureView
- `h-[133px]` gap-2.5, two `h-[100px] flex-1 rounded-[14px]
  border-[#1f1f1f] bg-[#121212]` buttons.
- **Snip:** Scissors + "Snip" — **inert today**; real: `ms-screenclip:`.
- **Record:** idle red ring (`h-6 w-6 border-[5px] border-[#ff453a]`) +
  "Record"; recording: pulsing glow (`h-14 w-14 bg-[#ff453a]/15 blur-xl`,
  .35→.85→.35, .8→1.25→.8, 1.7 s) + filled Square + `MM:SS` tabular.
  Mock: 1 s interval counter in Notch state; Record dissolves into the
  compact bar after 450 ms when `compactRecording` is on.

### SettingsView
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
- "COMPACT BAR" section: master `compactBar` ("Show compact bar") plus
  indented children `compactMusic`, `compactTimer`, `compactRecording`,
  `compactCharging` (Music/Timer/Recording/Charging). Children dim when the
  master is off; all toggles apply live.
- "CALENDARS" + "＋ Add" → connect sheet; account rows; "Notes folder"
  `h-9 rounded-xl bg-[#121212] px-2.5` label + path ("Documents\Notch
  Notes" mock text — spec folder is `Documents\Float Notes`) + "Open folder"
  pill (inert → Explorer).
- Settings apply live through `settingsService`; real:
  `%APPDATA%\Float\settings.json`.

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
