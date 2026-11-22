import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { debugLog } from "./debugLog";
import { registerTauriListener } from "./listenerRegistry";
import type { NotchSettings, PowerState, TimerSnapshot, Track } from "./types";

type AppSettings = {
  notch: NotchSettings;
  notesFolder: string | null;
  calendarAccounts: unknown[];
  timerCustomTimes: number[];
  timerDurationMs: number;
  timerLastCustomMs: number | null;
  timerSound: boolean;
};

/** Last value read from disk, so unrelated fields survive a notch-only save. */
let lastLoaded: AppSettings | null = null;

async function loadRaw(): Promise<AppSettings> {
  const settings = await invoke<AppSettings>("settings_get");
  lastLoaded = settings;
  return settings;
}

async function load(): Promise<NotchSettings> {
  return (await loadRaw()).notch;
}

async function save(settings: NotchSettings): Promise<void> {
  const base = lastLoaded ?? (await loadRaw());
  const next: AppSettings = { ...base, notch: settings };
  await invoke("settings_set", { settings: next });
  lastLoaded = next;
}

export const settingsService = { load, save };

/** Rust emits `power-state` only on AC/percent changes, never at startup. */
let powerStarted: Promise<void> | null = null;
const powerListeners = new Set<(state: PowerState) => void>();

function ensurePowerStarted(): Promise<void> {
  if (!powerStarted) {
    powerStarted = registerTauriListener(
      listen<PowerState>("power-state", (event) => {
        for (const listener of powerListeners) listener(event.payload);
      }),
    ).then(() => {
      debugLog("[ui] listener registered: power-state");
    });
  }
  return powerStarted;
}

export const powerService = {
  subscribe: async (handler: (state: PowerState) => void): Promise<() => void> => {
    await ensurePowerStarted();
    powerListeners.add(handler);
    return () => {
      powerListeners.delete(handler);
    };
  },
};

/** Rust `media-state` / `media_snapshot` payload (docs/ARCHITECTURE.md). */
type MediaPayload = {
  title: string;
  artist: string;
  source: string;
  playing: boolean;
  playbackRate: number;
  positionMs: number;
  durationMs: number | null;
  artDataUrl: string | null;
  isMusicApp: boolean;
  albumTitle: string | null;
  capturedAtMs: number;
};

let mediaStarted: Promise<void> | null = null;
let mediaEventSeen = false;
let currentTrack: Track | null = null;
const mediaListeners = new Set<(track: Track | null) => void>();

function toTrack(payload: MediaPayload): Track {
  return {
    id: `${payload.title}|${payload.artist}|${payload.source}`,
    title: payload.title,
    artist: payload.artist,
    source: payload.source,
    // `durationMs: null` means "unknown"; MusicCard hides the bar for it.
    duration: payload.durationMs == null ? 0 : payload.durationMs / 1000,
    durationMs: payload.durationMs,
    colors: ["#3a3a3a", "#242424", "#141414"],
    artDataUrl: payload.artDataUrl,
    isMusicApp: payload.isMusicApp,
    albumTitle: payload.albumTitle,
    playing: payload.playing,
    playbackRate: payload.playbackRate,
    positionMs: payload.positionMs,
    capturedAtMs: payload.capturedAtMs,
  };
}

function publishMedia(track: Track | null): void {
  currentTrack = track;
  for (const listener of mediaListeners) listener(track);
}

function isOlderThanCurrent(capturedAtMs: number | undefined): boolean {
  return (
    capturedAtMs != null &&
    currentTrack?.capturedAtMs != null &&
    capturedAtMs < currentTrack.capturedAtMs
  );
}

/** Listener first, then the initial snapshot, so no state change is missed. */
function ensureMediaStarted(): Promise<void> {
  if (mediaStarted) return mediaStarted;
  mediaStarted = (async () => {
    await registerTauriListener(
      listen<MediaPayload | null>("media-state", (event) => {
        // Pushed events are newer than any in-flight snapshot response.
        mediaEventSeen = true;
        if (!event.payload) {
          publishMedia(null);
          return;
        }
        if (isOlderThanCurrent(event.payload.capturedAtMs)) return;
        publishMedia(toTrack(event.payload));
      }),
    );
    debugLog("[ui] listener registered: media-state");

    try {
      const snapshot = await invoke<MediaPayload | null>("media_snapshot");
      // A `media-state` event may have landed while the command was in flight.
      if (mediaEventSeen) return;
      publishMedia(snapshot ? toTrack(snapshot) : null);
    } catch (error) {
      console.error("media_snapshot failed", error);
    }
  })();
  return mediaStarted;
}

export const mediaService = {
  /** Real mode never shows mock songs. */
  getTracks: (): Track[] => [],
  getSnapshot: async (): Promise<Track | null> => {
    await ensureMediaStarted();
    return currentTrack;
  },
  subscribe: async (handler: (track: Track | null) => void): Promise<() => void> => {
    await ensureMediaStarted();
    mediaListeners.add(handler);
    handler(currentTrack);
    return () => {
      mediaListeners.delete(handler);
    };
  },
  /** Rust runs these on the music thread and returns SMTC's bool result. */
  toggle: (): Promise<boolean> => command("media_toggle"),
  next: (): Promise<boolean> => command("media_next"),
  prev: (): Promise<boolean> => command("media_prev"),
  seek: (positionMs: number): Promise<boolean> =>
    command("media_seek", { positionMs: Math.round(positionMs) }),
};

async function command(name: string, args?: Record<string, unknown>): Promise<boolean> {
  try {
    return await invoke<boolean>(name, args);
  } catch (error) {
    console.error(`${name} failed`, error);
    return false;
  }
}

/**
 * Rust-authoritative timer. `timer-state` carries the full snapshot; commands
 * return the same payload, so whichever arrives last wins (`capturedAtMs`).
 */
const TIMER_PRESETS = [5, 10, 25];

let timerStarted: Promise<void> | null = null;
let timerSnapshot: TimerSnapshot | null = null;
const timerListeners = new Set<(snapshot: TimerSnapshot) => void>();

function publishTimer(snapshot: TimerSnapshot): void {
  if (timerSnapshot && snapshot.capturedAtMs < timerSnapshot.capturedAtMs) return;
  timerSnapshot = snapshot;
  for (const listener of timerListeners) listener(snapshot);
}

function ensureTimerStarted(): Promise<void> {
  if (timerStarted) return timerStarted;
  timerStarted = (async () => {
    await registerTauriListener(
      listen<TimerSnapshot>("timer-state", (event) => publishTimer(event.payload)),
    );
    debugLog("[ui] listener registered: timer-state");

    try {
      publishTimer(await invoke<TimerSnapshot>("timer_get"));
    } catch (error) {
      console.error("timer_get failed", error);
    }
  })();
  return timerStarted;
}

async function timerCommand(
  name: string,
  args?: Record<string, unknown>,
): Promise<TimerSnapshot | null> {
  try {
    const snapshot = await invoke<TimerSnapshot>(name, args);
    publishTimer(snapshot);
    return snapshot;
  } catch (error) {
    console.error(`${name} failed`, error);
    return null;
  }
}

export const timerService = {
  getPresets: (): number[] => TIMER_PRESETS,
  get: async (): Promise<TimerSnapshot | null> => {
    await ensureTimerStarted();
    return timerSnapshot;
  },
  subscribe: async (handler: (snapshot: TimerSnapshot) => void): Promise<() => void> => {
    await ensureTimerStarted();
    if (timerSnapshot) handler(timerSnapshot);
    timerListeners.add(handler);
    return () => {
      timerListeners.delete(handler);
    };
  },
  set: (durationMs: number): Promise<TimerSnapshot | null> =>
    timerCommand("timer_set", { durationMs: Math.round(durationMs) }),
  start: (): Promise<TimerSnapshot | null> => timerCommand("timer_start"),
  pause: (): Promise<TimerSnapshot | null> => timerCommand("timer_pause"),
  resume: (): Promise<TimerSnapshot | null> => timerCommand("timer_resume"),
  reset: (): Promise<TimerSnapshot | null> => timerCommand("timer_reset"),
  adjust: (deltaMs: number): Promise<TimerSnapshot | null> =>
    timerCommand("timer_adjust", { deltaMs: Math.round(deltaMs) }),
  restart: (): Promise<TimerSnapshot | null> => timerCommand("timer_restart"),
};

/** Notes service: real mode uses Tauri commands, mock mode uses localStorage. */
export const notesService = {
  /** Real mode: list notes from the notes folder. */
  list: async (): Promise<Array<{ id: string; title: string; modifiedMs: number }>> => {
    if (import.meta.env.VITE_USE_MOCK) {
      return mockNoteList();
    }
    return invoke<Array<{ id: string; title: string; modifiedMs: number }>>("notes_list")
      .then((results) => results.sort((a, b) => b.modifiedMs - a.modifiedMs));
  },
  /** Real mode: get a note by id. */
  get: async (id: string): Promise<{ id: string; title: string; body: string; modifiedMs: number }> => {
    if (import.meta.env.VITE_USE_MOCK) {
      return mockNoteGet(id);
    }
    return invoke<{ id: string; title: string; body: string; modifiedMs: number }>("notes_get", { id })
      .then((note) => note);
  },
  /** Real mode: save a note. */
  save: async (id: string | null, title: string, body: string): Promise<{ id: string | null; modifiedMs: number }> => {
    if (import.meta.env.VITE_USE_MOCK) {
      return mockNoteSave(id, title, body);
    }
    return invoke<{ id: string | null; modifiedMs: number }>("notes_save", { id, title, body });
  },
  /** Real mode: delete a note. */
  delete: async (id: string): Promise<void> => {
    if (import.meta.env.VITE_USE_MOCK) {
      mockNoteDelete(id);
      return;
    }
    await invoke<void>("notes_delete", { id });
  },
  /** Real mode: open the notes folder. */
  openFolder: async (): Promise<void> => {
    if (import.meta.env.VITE_USE_MOCK) {
      // no-op in mock mode
      return;
    }
    await invoke<void>("notes_open_folder");
  },
  /** Real mode: get the notes folder path. */
  folderPath: async (): Promise<string> => {
    if (import.meta.env.VITE_USE_MOCK) {
      return "Documents/Float Notes";
    }
    return invoke<string>("notes_folder_path");
  },
};

/** Mock note operations using localStorage. */
function mockNoteList(): Array<{ id: string; title: string; modifiedMs: number }> {
  return [];
}

function mockNoteGet(id: string): Promise<{ id: string; title: string; body: string; modifiedMs: number }> {
  return Promise.resolve({ id, title: "", body: "", modifiedMs: 0 });
}

function mockNoteSave(id: string | null, title: string, body: string): Promise<{ id: string | null; modifiedMs: number }> {
  return Promise.resolve({ id, modifiedMs: Date.now() });
}

function mockNoteDelete(id: string): void {
  // no-op in mock mode
}
