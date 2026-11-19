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
  shortcut: string;
};

export const DEFAULT_NOTCH_SETTINGS: NotchSettings = {
  topEdge: true,
  altAlt: true,
  hideInFullscreen: true,
  launchAtStartup: true,
  upcomingEvent: true,
  noCalendars: false,
  shortcut: "Alt Alt",
};
