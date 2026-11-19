import type { LiveActivity } from "./types";

/**
 * Highest priority wins. Ties go to the activity that started most recently,
 * which the store encodes as the later entry in the registration-ordered list.
 */
export function pickActivity(activities: readonly LiveActivity[]): LiveActivity | null {
  let picked: LiveActivity | null = null;
  for (const activity of activities) {
    if (!picked || activity.priority >= picked.priority) picked = activity;
  }
  return picked;
}
