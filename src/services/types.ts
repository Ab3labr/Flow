export type Track = {
  id: string;
  title: string;
  artist: string;
  source: string;
  duration: number;
  colors: [string, string, string];
  /** Real SMTC fields (absent on mock tracks). */
  artDataUrl?: string | null;
  playing?: boolean;
  playbackRate?: number;
  positionMs?: number;
  durationMs?: number | null;
  capturedAtMs?: number;
  /** Compact-bar gate from Rust; the full panel still shows any media. */
  isMusicApp: boolean;
  albumTitle: string | null;
};

export type TimerStatus = "idle" | "running" | "paused" | "done";

/**
 * Rust `timer-state` payload. Times are milliseconds; `endsAtMs` exists only
 * while running so the frontend can count down locally.
 */
export type TimerSnapshot = {
  state: TimerStatus;
  durationMs: number;
  totalMs: number;
  remainingMs: number;
  capturedAtMs: number;
  endsAtMs?: number;
  /** Last non-preset duration that was started, if any. */
  lastCustomMs: number | null;
};

export type PowerTransition = "plugged" | "unplugged" | "none";

/** Rust `power-state` payload. `percent: null` when Windows reports 255. */
export type PowerState = {
  pluggedIn: boolean;
  charging: boolean;
  percent: number | null;
  transition: PowerTransition;
};

/** Rust `privacy-state` payload: local HKCU usage-timestamp starts only. */
export type PrivacyState = {
  kinds: ("microphone" | "camera")[];
  app: string;
};

export type CalendarEvent = {
  id: string;
  accountId: string;
  title: string;
  detail: string;
  time: string;
  color: string;
  startsAt: number;
  location?: string;
  description?: string;
  live?: boolean;
};

export type CalendarProvider = "google" | "outlook" | "icloud" | "ics";

export type CalendarAccount = {
  id: string;
  provider: CalendarProvider;
  name: string;
  email: string;
  color: string;
  visible: boolean;
  status: "synced" | "error";
};

export type Note = {
  id: string;
  title: string;
  body: string;
  updatedAt: number;
};

export type NotchSettings = {
  topEdge: boolean;
  altAlt: boolean;
  hideInFullscreen: boolean;
  launchAtStartup: boolean;
  upcomingEvent: boolean;
  noCalendars: boolean;
  compactBar: boolean;
  compactMusic: boolean;
  compactTimer: boolean;
  compactRecording: boolean;
  compactCharging: boolean;
  compactPrivacy: boolean;
  shortcut: string;
  hotkeysEnabled: boolean;
  hotkeyNotes: string;
  hotkeyNext: string;
  hotkeyPrev: string;
  /** Minutes before Notes opens a fresh draft; 0 = never. */
  notesNewAfterMinutes: number;
  /** Last note shown, used to reopen it after a restart. */
  notesLastId: string | null;
  /** Last edit timestamp for `notesLastId`. */
  notesLastEditedMs: number;
};

export const DEFAULT_NOTCH_SETTINGS: NotchSettings = {
  topEdge: true,
  altAlt: true,
  hideInFullscreen: true,
  launchAtStartup: true,
  upcomingEvent: true,
  noCalendars: false,
  compactBar: true,
  compactMusic: true,
  compactTimer: true,
  compactRecording: true,
  compactCharging: true,
  compactPrivacy: true,
  shortcut: "Alt Alt",
  hotkeysEnabled: true,
  hotkeyNotes: "Alt+N",
  hotkeyNext: "Alt+C",
  hotkeyPrev: "Alt+Z",
  notesNewAfterMinutes: 720,
  notesLastId: null,
  notesLastEditedMs: 0,
};
