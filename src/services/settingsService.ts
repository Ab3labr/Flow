import { invoke } from "@tauri-apps/api/core";
import { isMockMode } from "./env";

export type NotchSettings = {
  topEdge: boolean;
  altAlt: boolean;
  fullscreen: boolean;
  launch: boolean;
  upcomingEvent: boolean;
  noCalendars: boolean;
  shortcut: string;
};

export const DEFAULT_NOTCH_SETTINGS: NotchSettings = {
  topEdge: true,
  altAlt: true,
  fullscreen: true,
  launch: true,
  upcomingEvent: true,
  noCalendars: false,
  shortcut: "Alt Alt",
};

type BackendNotchSettings = {
  topEdge: boolean;
  altAlt: boolean;
  hideInFullscreen: boolean;
  launchAtStartup: boolean;
  upcomingEvent: boolean;
  noCalendars: boolean;
  shortcut: string;
};

type BackendSettings = {
  notch: BackendNotchSettings;
  notesFolder: string | null;
  calendarAccounts: unknown[];
  timerCustomTimes: number[];
};

const DEFAULT_BACKEND_SETTINGS: BackendSettings = {
  notch: {
    topEdge: true,
    altAlt: true,
    hideInFullscreen: true,
    launchAtStartup: false,
    upcomingEvent: true,
    noCalendars: false,
    shortcut: "Alt Alt",
  },
  notesFolder: null,
  calendarAccounts: [],
  timerCustomTimes: [],
};

const MOCK_STORAGE_KEY = "float-mock-settings";

function fromBackend(settings: BackendNotchSettings): NotchSettings {
  return {
    topEdge: settings.topEdge,
    altAlt: settings.altAlt,
    fullscreen: settings.hideInFullscreen,
    launch: settings.launchAtStartup,
    upcomingEvent: settings.upcomingEvent,
    noCalendars: settings.noCalendars,
    shortcut: settings.shortcut,
  };
}

function toBackend(settings: NotchSettings): BackendNotchSettings {
  return {
    topEdge: settings.topEdge,
    altAlt: settings.altAlt,
    hideInFullscreen: settings.fullscreen,
    launchAtStartup: settings.launch,
    upcomingEvent: settings.upcomingEvent,
    noCalendars: settings.noCalendars,
    shortcut: settings.shortcut,
  };
}

function readMockBackend(): BackendSettings {
  try {
    const stored = JSON.parse(localStorage.getItem(MOCK_STORAGE_KEY) ?? "null");
    if (stored?.notch) {
      return {
        ...DEFAULT_BACKEND_SETTINGS,
        ...stored,
        notch: { ...DEFAULT_BACKEND_SETTINGS.notch, ...stored.notch },
      };
    }
  } catch {
    // Corrupt mock value: fall back to defaults.
  }
  return { ...DEFAULT_BACKEND_SETTINGS, notch: { ...DEFAULT_BACKEND_SETTINGS.notch } };
}

/** Last value read from disk, so unrelated fields survive a notch-only save. */
let lastLoaded: BackendSettings | null = null;

async function loadRaw(): Promise<BackendSettings> {
  if (isMockMode) return readMockBackend();
  const settings = await invoke<BackendSettings>("settings_get");
  lastLoaded = settings;
  return settings;
}

async function load(): Promise<NotchSettings> {
  return fromBackend((await loadRaw()).notch);
}

async function save(settings: NotchSettings): Promise<void> {
  const base = lastLoaded ?? (await loadRaw());
  const next: BackendSettings = { ...base, notch: toBackend(settings) };
  if (isMockMode) {
    localStorage.setItem(MOCK_STORAGE_KEY, JSON.stringify(next));
    lastLoaded = next;
    return;
  }
  await invoke("settings_set", { settings: next });
  lastLoaded = next;
}

export const settingsService = { load, save };
