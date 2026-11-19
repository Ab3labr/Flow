import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { NotchSettings, Track } from "./types";

type AppSettings = {
  notch: NotchSettings;
  notesFolder: string | null;
  calendarAccounts: unknown[];
  timerCustomTimes: number[];
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

/** Rust `media-state` / `media_snapshot` payload (docs/ARCHITECTURE.md). */
type MediaPayload = {
  title: string;
  artist: string;
  source: string;
  playing: boolean;
  positionMs: number;
  durationMs: number | null;
  artDataUrl: string | null;
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
    playing: payload.playing,
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
    await listen<MediaPayload | null>("media-state", (event) => {
      // Pushed events are newer than any in-flight snapshot response.
      mediaEventSeen = true;
      if (!event.payload) {
        publishMedia(null);
        return;
      }
      if (isOlderThanCurrent(event.payload.capturedAtMs)) return;
      publishMedia(toTrack(event.payload));
    });

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
};
