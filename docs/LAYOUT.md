# LAYOUT — dimensions, colors, type (as built)

Reference stage: 1920x1080 (prototype only). All values are CSS pixels;
in Tauri use logical px (same numbers at 100% scaling).

## Panel
- Expanded width: **480 px**. Heights per tab:
  home 172, timer 250, calendar 230, capture 185, settings 340,
  notes dynamic **172 → 340**.
- Collapsed: invisible sliver **120x4** (opacity 0, radius 2); live pill
  **108x26** (radius 13).
- Position: top-center of screen, top edge y=0.
- Background `#000` (`bg-black`), `overflow-hidden`.
- Shadow when open/pill: `0 12px 34px rgba(0,0,0,.36)`.
- Border radius: bottom corners only — expanded **22 px**, pill **13 px**,
  sliver 2 px. (Top corners square — it hangs from screen top.)
- Content padding: `px-2.5 pt-2.5` (10 px sides/top); TopBar `h-[26px]`;
  content `mt-1.5` (6 px).

## Window size recommendation (Tauri)
Max panel 480x340 + shadow bleed (~40 px) → window ≈ **560x400** logical,
transparent, centered horizontally on the active monitor, y=0. Pill mode
can shrink to ~200x90; see docs/ARCHITECTURE.md.

## Colors
- Page/stage bg: `#02080d` outer, `#06131b` stage (prototype only).
- Cards: `bg-[#121212]`, borders `border-[#1f1f1f]` (also `border-white/10`
  on popovers).
- Sub-surfaces / hover chips: `bg-[#1c1c1c]`; popovers `bg-[#202020]`.
- Text: white; secondary `white/60`, `white/65`, `white/70`, `white/75`,
  `white/80`; faint `white/35`, `white/40`, `white/45`, `white/50`;
  hairlines `white/5`, `white/6`, `white/10`, `white/15`.
- Accents: red `#ff453a` (rec dot, date label, sync error), green
  `#30d158` (Join), pink `#f09ab7` (equalizer), timer ring track `#232323`,
  ring progress white.
- Track gradient colors come from data (`Track.colors` triple, 135°).
- System-ish blues only in fake taskbar (`#57a8ef`) — prototype only.

## Type
- Font: Inter (Google Fonts `@import` in index.css) with fallback stack
  `ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif`.
- `color-scheme: dark`; body `overflow:hidden`, `user-select:none`.
- Scale used: 30px semibold (idle clock), 25px semibold tabular (timer),
  14px semibold (note title), 13px semibold/medium (titles, buttons),
  12px (idle date), 11px (most UI: labels, pills, rows, mono-ish times),
  10px (fake taskbar clock only).
- Labels: `font-bold uppercase tracking-[.08em]` or `.04em` white/40.
- Numerals: `tabular-nums` everywhere times/countdowns appear.
- Letter-spacing `-.04em` on large numerals.

## Key component metrics
- TopBar tab button: `h-6 w-6` (24px), icon 15px strokeWidth 1.8;
  active pill inset-0 rounded-full.
- PressButton: spring whileTap 0.92 (stiffness 600 damping 30).
- MusicCard: art 48x48 radius 11; glow `-left-5 -top-7` 112x160 blur 60
  opacity .12; controls row h-22px; play button 26px, skip/lyrics/ellipsis
  22px; volume popover w-40.
- Progress track: h-1 (hover h-1.5, 150 ms), thumb dot 10px.
- Timer ring: 128px box, r=63 sw=6 in 144 viewBox; handle 12px dot border
  2px black; HoldButton 28px; presets h-7 px-3; custom pills same;
  Start button h-9 full-width-of-column.
- Calendar rows: h-42px, radius 12 (`rounded-xl`), color bar 3x24 radius
  full, time col w-52px, list h-138px (96px when error banner).
  EventCard (home): w-148px, radius 14, inner block h-58 radius 9.
- Connect sheet: full overlay radius 14, p-3; provider rows h-7; signin
  block h-130 centered.
- Notes card: radius 14, footer h-6, paddings px-3 pt-2.5; recent popover
  w-56, rows h-10, radius 12 (`rounded-lg` rows inside rounded-xl).
- Capture tiles: h-100 flex-1 radius 14; record ring 24px border 5px.
- Settings rows: h-8, radius 14 card, radius 12 (`rounded-xl`) sections;
  toggle 36x20 (w-9 h-5), knob 14px at x 3/19, top 3px.
- Live pill content: h-26px, px-2.5, gap-2, text 11px medium tabular.
- Equalizer bars: w-2px, pink, gap 2px, container h-3.

## Spacing rhythm
- Panel padding 10px; section gaps mt-1.5 (6px), mt-2 (8px), mt-2.5 (10px),
  mt-3 (12px); card inner padding mostly p-2.5 (10px) or px-3 pt-2.5.
- Popovers offset `bottom-7/8` above their buttons.

## Prototype-only chrome (delete for real app)
- 1920x1080 stage scale wrapper, Unsplash bg + gradient overlays, fake
  taskbar h-48px bottom with Start/Search/system tray icons, fake clock
  "2:42 PM / 11/4/2026".
