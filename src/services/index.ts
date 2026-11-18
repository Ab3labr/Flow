import { isMockMode } from "./env";
import * as mock from "./mockServices";
import { settingsService as tauriSettingsService } from "./tauriServices";

export * from "./types";

// Only settings has a real implementation so far; the rest stay mock until
// their phases land.
export const mediaService = mock.mediaService;
export const calendarService = mock.calendarService;
export const timerService = mock.timerService;
export const captureService = mock.captureService;
export const noteService = mock.noteService;

export const settingsService = isMockMode ? mock.settingsService : tauriSettingsService;
