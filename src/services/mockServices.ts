import {
  DEFAULT_NOTCH_SETTINGS,
  type CalendarAccount,
  type CalendarEvent,
  type CalendarProvider,
  type Note,
  type NotchSettings,
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
  },
  {
    id: "afterglow",
    title: "Afterglow",
    artist: "Lunar Arcade",
    source: "YouTube",
    duration: 213,
    colors: ["#df506f", "#7347a4", "#232c6b"],
  },
  {
    id: "blue-hour",
    title: "Blue Hour Drive",
    artist: "Northbound",
    source: "Apple Music",
    duration: 268,
    colors: ["#4ac0b9", "#286e9e", "#3d285f"],
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

export const timerService = {
  getPresets: () => [5, 10, 25],
  getCustomTimes: () => {
    try {
      return JSON.parse(localStorage.getItem("notch-custom-times") || "[]") as number[];
    } catch {
      return [];
    }
  },
  rememberCustomTime: (minutes: number) => {
    const recent = timerService
      .getCustomTimes()
      .filter((value) => value !== minutes)
      .slice(0, 2);
    const next = [minutes, ...recent];
    localStorage.setItem("notch-custom-times", JSON.stringify(next));
    return next;
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
