/** Shared compact-bar geometry. Keep these pure for frontend tests. */
export const COMPACT_BAR_HEIGHT = 30;
export const COMPACT_BAR_MIN_WIDTH = 120;
export const COMPACT_BAR_MAX_WIDTH = 260;

/** Window margin used by Rust `set_ui_mode("pill")`. */
export const COMPACT_BAR_WINDOW_MARGIN = { x: 32, y: 18 } as const;

/** Hidden collapsed sliver; never shown as an intermediate state. */
export const IDLE_BAR_WIDTH = 120;
export const IDLE_BAR_HEIGHT = 4;
