import {
  DEFAULT_NOTCH_SETTINGS,
  type CalendarAccount,
  type CalendarEvent,
  type CalendarProvider,
  type Note,
  type NotchSettings,
  type PowerState,
  type TimerSnapshot,
  type Track,
} from "./types";

const tracks: Track[] = [
  {
    id: "midnight-city-lights",
    title: "Midnight City Lights",
    artist: "The Night Drivers",
    source: "Spotify",
    duration: 242,
    colors: ["#ff9a5a", "#d04a7d", "#553087"],
    isMusicApp: true,
    albumTitle: null,
  },
  {
    id: "afterglow",
    title: "Afterglow",
    artist: "Lunar Arcade",
    source: "YouTube",
    duration: 213,
    colors: ["#df506f", "#7347a4", "#232c6b"],
    isMusicApp: true,
    albumTitle: null,
  },
  {
    id: "blue-hour",
    title: "Blue Hour Drive",
    artist: "Northbound",
    source: "Apple Music",
    duration: 268,
    colors: ["#4ac0b9", "#286e9e", "#3d285f"],
    isMusicApp: true,
    albumTitle: null,
  },
];

const calendarAccounts: CalendarAccount[] = [
  {
    id: "google-work",
    provider: "google",
    name: "Google Calendar",
    email: "alex@studio.design",
    color: "#a970ff",
    visible: true,
    status: "synced",
  },
  {
    id: "outlook-work",
    provider: "outlook",
    name: "Outlook / Microsoft 365",
    email: "alex@northstar.co",
    color: "#5e98ff",
    visible: true,
    status: "synced",
  },
];

const mockEventStart = Date.now();
const rawEvents = [
  {
    id: "design-review",
    accountId: "google-work",
    title: "Design review",
    offsetMinutes: 5,
    duration: "30 min",
    location: "https://meet.google.com/notch-review",
    description: "Review the latest desktop prototype.",
    live: true,
  },
  {
    id: "product-sync",
    accountId: "outlook-work",
    title: "Product sync",
    offsetMinutes: 80,
    duration: "30 min",
    location: "https://teams.microsoft.com/l/meetup-join/notch",
    description: "Weekly product sync.",
  },
  {
    id: "focus-time",
    accountId: "google-work",
    title: "Focus time",
    offsetMinutes: 150,
    duration: "1 hour",
    location: "Studio",
    description: "Protected focus block.",
  },
];

export const mediaService = {
  getTracks: () => tracks,
  /** The browser prototype drives tracks locally; there is no real session. */
  getSnapshot: async (): Promise<Track | null> => null,
  subscribe: async (_handler: (track: Track | null) => void): Promise<() => void> =>
    () => {},
  toggle: async (): Promise<boolean> => false,
  next: async (): Promise<boolean> => false,
  prev: async (): Promise<boolean> => false,
  seek: async (_positionMs: number): Promise<boolean> => false,
};

export const calendarService = {
  getDefaultAccounts: () => calendarAccounts.map((account) => ({ ...account })),
  getProviders: () =>
    [
      { id: "google", label: "Google Calendar", action: "Continue with Google" },
      { id: "outlook", label: "Outlook / Microsoft 365", action: "Continue with Microsoft" },
      { id: "icloud", label: "Apple iCloud", action: "Continue with Apple" },
      { id: "ics", label: "Other calendar (ICS link)", action: "Add ICS calendar" },
    ] as const,
  createAccount: (provider: CalendarProvider, index: number): CalendarAccount => {
    const providerInfo = calendarService.getProviders().find((item) => item.id === provider)!;
    const colors = ["#a970ff", "#5e98ff", "#ff6b7a", "#30d158"];
    return {
      id: `${provider}-${Date.now()}`,
      provider,
      name: providerInfo.label,
      email:
        provider === "ics"
          ? `Shared calendar ${index + 1}`
          : `alex${index ? `+${index}` : ""}@${provider === "google" ? "gmail.com" : provider === "outlook" ? "northstar.co" : "icloud.com"}`,
      color: colors[index % colors.length],
      visible: true,
      status: "synced",
    };
  },
  getToday: (accounts: CalendarAccount[] = calendarAccounts): CalendarEvent[] =>
    rawEvents
      .filter((event) =>
        accounts.some(
          (account) =>
            account.id === event.accountId && account.visible && account.status === "synced",
        ),
      )
      .map((event) => {
        const account = accounts.find((item) => item.id === event.accountId)!;
        const startsAt = mockEventStart + event.offsetMinutes * 60_000;
        return {
          id: event.id,
          accountId: event.accountId,
          title: event.title,
          detail: `${event.offsetMinutes === 5 ? "in 5 min" : new Date(startsAt).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })} · ${event.duration}`,
          time: new Date(startsAt).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }),
          startsAt,
          color: account.color,
          location: event.location,
          description: event.description,
          live: event.live,
        };
      })
      .sort((a, b) => a.startsAt - b.startsAt),
  getMeetingProvider: (event: CalendarEvent) => {
    const content = `${event.location || ""} ${event.description || ""}`.toLowerCase();
    if (content.includes("meet.google.com")) return "Google Meet";
    if (content.includes("zoom.us")) return "Zoom";
    if (content.includes("teams.microsoft.com")) return "Teams";
    return null;
  },
};

const TIMER_PRESETS = [5, 10, 25];
const TIMER_MIN_MS = 1_000;
const TIMER_MAX_MS = 86_399_000;
const TIMER_DURATION_KEY = "notch-timer-duration";
const TIMER_LAST_CUSTOM_KEY = "notch-timer-last-custom";

type MockTimerState =
  | { kind: "idle"; durationMs: number }
  | { kind: "running"; endsAtMs: number; totalMs: number }
  | { kind: "paused"; remainingMs: number; totalMs: number }
  | { kind: "done"; totalMs: number };

function clampDuration(ms: number): number {
  return Math.min(TIMER_MAX_MS, Math.max(TIMER_MIN_MS, Math.round(ms)));
}

function offsetDuration(value: number, deltaMs: number): number {
  return Math.max(0, value + deltaMs);
}

function readStoredDuration(): number {
  try {
    const stored = Number(localStorage.getItem(TIMER_DURATION_KEY));
    return Number.isFinite(stored) && stored > 0 ? clampDuration(stored) : 25 * 60_000;
  } catch {
    return 25 * 60_000;
  }
}

function readStoredLastCustom(): number | null {
  try {
    const stored = Number(localStorage.getItem(TIMER_LAST_CUSTOM_KEY));
    return Number.isFinite(stored) && stored > 0 ? clampDuration(stored) : null;
  } catch {
    return null;
  }
}

function mockTotalMs(state: MockTimerState): number {
  return state.kind === "idle" ? state.durationMs : state.totalMs;
}

function mockRemainingMs(state: MockTimerState, now: number): number {
  switch (state.kind) {
    case "idle":
      return state.durationMs;
    case "running":
      return Math.min(state.totalMs, Math.max(0, state.endsAtMs - now));
    case "paused":
      return state.remainingMs;
    case "done":
      return 0;
  }
}

let mockTimerState: MockTimerState = { kind: "idle", durationMs: readStoredDuration() };
let mockLastCustomMs: number | null = readStoredLastCustom();
let mockTimerTicker: number | undefined;
const mockTimerListeners = new Set<(snapshot: TimerSnapshot) => void>();

function mockTimerSnapshot(now = Date.now()): TimerSnapshot {
  const state = mockTimerState;
  const totalMs = mockTotalMs(state);
  const snapshot: TimerSnapshot = {
    state: state.kind,
    durationMs: state.kind === "idle" ? state.durationMs : totalMs,
    totalMs,
    remainingMs: mockRemainingMs(state, now),
    capturedAtMs: now,
    lastCustomMs: mockLastCustomMs,
  };
  if (state.kind === "running") snapshot.endsAtMs = state.endsAtMs;
  return snapshot;
}

function publishMockTimer(): void {
  const snapshot = mockTimerSnapshot();
  for (const listener of mockTimerListeners) listener(snapshot);
}

function stopMockTicker(): void {
  if (mockTimerTicker !== undefined) window.clearInterval(mockTimerTicker);
  mockTimerTicker = undefined;
}

/** Mock stand-in for the Rust 250 ms tick thread: flips once at zero. */
function ensureMockTicker(): void {
  if (mockTimerState.kind !== "running") {
    stopMockTicker();
    return;
  }
  if (mockTimerTicker !== undefined) return;
  mockTimerTicker = window.setInterval(() => {
    const state = mockTimerState;
    if (state.kind === "running" && Date.now() >= state.endsAtMs) {
      mockTimerState = { kind: "done", totalMs: state.totalMs };
      stopMockTicker();
      publishMockTimer();
    }
  }, 250);
}

function rememberMockCustom(totalMs: number): void {
  if (TIMER_PRESETS.includes(totalMs / 60_000)) return;
  mockLastCustomMs = totalMs;
  try {
    localStorage.setItem(TIMER_LAST_CUSTOM_KEY, String(totalMs));
  } catch {
    // Mock only; storage may be unavailable in private windows.
  }
}

export const timerService = {
  getPresets: (): number[] => TIMER_PRESETS,
  get: async (): Promise<TimerSnapshot> => mockTimerSnapshot(),
  subscribe: async (handler: (snapshot: TimerSnapshot) => void): Promise<() => void> => {
    mockTimerListeners.add(handler);
    handler(mockTimerSnapshot());
    return () => {
      mockTimerListeners.delete(handler);
    };
  },
  set: async (durationMs: number): Promise<TimerSnapshot> => {
    const clamped = clampDuration(durationMs);
    if (mockTimerState.kind === "idle") mockTimerState = { kind: "idle", durationMs: clamped };
    try {
      localStorage.setItem(TIMER_DURATION_KEY, String(clamped));
    } catch {
      // Ignore storage failures in the prototype.
    }
    publishMockTimer();
    return mockTimerSnapshot();
  },
  start: async (): Promise<TimerSnapshot> => {
    if (mockTimerState.kind === "idle") {
      const totalMs = mockTimerState.durationMs;
      mockTimerState = { kind: "running", endsAtMs: Date.now() + totalMs, totalMs };
      rememberMockCustom(totalMs);
      ensureMockTicker();
      publishMockTimer();
    }
    return mockTimerSnapshot();
  },
  pause: async (): Promise<TimerSnapshot> => {
    if (mockTimerState.kind === "running") {
      const { endsAtMs, totalMs } = mockTimerState;
      mockTimerState = {
        kind: "paused",
        remainingMs: Math.min(totalMs, Math.max(0, endsAtMs - Date.now())),
        totalMs,
      };
      stopMockTicker();
      publishMockTimer();
    }
    return mockTimerSnapshot();
  },
  resume: async (): Promise<TimerSnapshot> => {
    if (mockTimerState.kind === "paused") {
      const { remainingMs, totalMs } = mockTimerState;
      mockTimerState = { kind: "running", endsAtMs: Date.now() + remainingMs, totalMs };
      ensureMockTicker();
      publishMockTimer();
    }
    return mockTimerSnapshot();
  },
  reset: async (): Promise<TimerSnapshot> => {
    mockTimerState = { kind: "idle", durationMs: clampDuration(mockTotalMs(mockTimerState)) };
    stopMockTicker();
    publishMockTimer();
    return mockTimerSnapshot();
  },
  adjust: async (deltaMs: number): Promise<TimerSnapshot> => {
    const now = Date.now();
    const state = mockTimerState;
    if (state.kind === "idle") {
      mockTimerState = { kind: "idle", durationMs: clampDuration(offsetDuration(state.durationMs, deltaMs)) };
      try {
        localStorage.setItem(TIMER_DURATION_KEY, String(mockTimerState.durationMs));
      } catch {
        // Ignore storage failures in the prototype.
      }
    } else if (state.kind === "running") {
      mockTimerState = {
        kind: "running",
        endsAtMs: Math.max(now, state.endsAtMs + deltaMs),
        totalMs: clampDuration(offsetDuration(state.totalMs, deltaMs)),
      };
    } else if (state.kind === "paused") {
      mockTimerState = {
        kind: "paused",
        remainingMs: clampDuration(offsetDuration(state.remainingMs, deltaMs)),
        totalMs: clampDuration(offsetDuration(state.totalMs, deltaMs)),
      };
    }
    publishMockTimer();
    return mockTimerSnapshot();
  },
  restart: async (): Promise<TimerSnapshot> => {
    if (mockTimerState.kind === "done") {
      const totalMs = mockTimerState.totalMs;
      mockTimerState = { kind: "running", endsAtMs: Date.now() + totalMs, totalMs };
      ensureMockTicker();
      publishMockTimer();
    }
    return mockTimerSnapshot();
  },
};

export const captureService = {
  getModes: () => ["Snip", "Record"] as const,
};

const NOTES_KEY = "notch-notes";

function readNotes(): Note[] {
  try {
    return JSON.parse(localStorage.getItem(NOTES_KEY) || "[]") as Note[];
  } catch {
    return [];
  }
}

function writeNotes(notes: Note[]) {
  localStorage.setItem(NOTES_KEY, JSON.stringify(notes));
}

export const noteService = {
  list: () => readNotes().sort((a, b) => b.updatedAt - a.updatedAt),
  get: (id: string) => readNotes().find((note) => note.id === id),
  create: () => {
    const note: Note = {
      id: crypto.randomUUID(),
      title: "",
      body: "",
      updatedAt: Date.now(),
    };
    writeNotes([note, ...readNotes()]);
    return note;
  },
  save: (note: Note) => {
    const notes = readNotes().filter((item) => item.id !== note.id);
    const saved = { ...note, updatedAt: Date.now() };
    writeNotes([saved, ...notes]);
    return saved;
  },
  remove: (id: string) => {
    writeNotes(readNotes().filter((note) => note.id !== id));
  },
};

const MOCK_SETTINGS_KEY = "float-mock-notch-settings";

function readMockSettings(): NotchSettings {
  try {
    const stored = JSON.parse(localStorage.getItem(MOCK_SETTINGS_KEY) ?? "null");
    if (stored) return { ...DEFAULT_NOTCH_SETTINGS, ...stored };
  } catch {
    // Corrupt mock value: fall back to defaults.
  }
  return { ...DEFAULT_NOTCH_SETTINGS };
}

export const settingsService = {
  load: async (): Promise<NotchSettings> => readMockSettings(),
  save: async (settings: NotchSettings): Promise<void> => {
    localStorage.setItem(MOCK_SETTINGS_KEY, JSON.stringify(settings));
  },
};

/** No power events in the browser prototype. */
export const powerService = {
  subscribe: async (_handler: (state: PowerState) => void): Promise<() => void> => () => {},
};
