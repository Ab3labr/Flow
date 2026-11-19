import { isMockMode } from "./env";
import * as mock from "./mockServices";
import {
  mediaService as tauriMediaService,
  powerService as tauriPowerService,
  settingsService as tauriSettingsService,
} from "./tauriServices";

export * from "./types";

// Settings, now-playing and power events have real implementations so far;
// the rest stay mock until their phases land.
export const mediaService = isMockMode ? mock.mediaService : tauriMediaService;
export const powerService = isMockMode ? mock.powerService : tauriPowerService;
export const calendarService = mock.calendarService;
export const timerService = mock.timerService;
export const captureService = mock.captureService;
export const noteService = mock.noteService;

export const settingsService = isMockMode ? mock.settingsService : tauriSettingsService;
