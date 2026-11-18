import { invoke } from "@tauri-apps/api/core";
import type { NotchSettings } from "./types";

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
