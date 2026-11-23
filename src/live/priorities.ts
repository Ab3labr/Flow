/** Shared activity priorities (docs/FEATURES.md, Phases 3D-3/3D-4). */
export const LIVE_PRIORITIES = {
  recording: 100,
  privacy: 97,
  charging: 95,
  timerDone: 90,
  meetingReminder: 80,
  timer: 60,
  music: 10,
} as const;
