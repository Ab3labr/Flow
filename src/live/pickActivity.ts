import type { LiveActivity } from "./types";

/**
 * Any live transient outranks every persistent slot. Within each group the
 * highest priority wins; ties go to the most recently registered activity,
 * which the store encodes as the later entry in the ordered list.
 */
export function pickActivity(activities: readonly LiveActivity[]): LiveActivity | null {
  let picked: LiveActivity | null = null;
  let pickedTransient = false;

  for (const activity of activities) {
    const transient = activity.mode === "transient";
    if (
      !picked ||
      (transient && !pickedTransient) ||
      (transient === pickedTransient && activity.priority >= picked.priority)
    ) {
      picked = activity;
      pickedTransient = transient;
    }
  }

  return picked;
}
