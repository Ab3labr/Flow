import { useSyncExternalStore } from "react";
import { pickActivity } from "./pickActivity";
import type { LiveActivity, LiveAction, LiveLeading, LiveTrailing } from "./types";

let activities: LiveActivity[] = [];
let snapshot: readonly LiveActivity[] = activities;
const listeners = new Set<() => void>();
const expiryTimers = new Map<string, number>();

function publish(next: LiveActivity[]): void {
  activities = next;
  snapshot = next;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot(): readonly LiveActivity[] {
  return snapshot;
}

function clearExpiry(id: string): void {
  const timer = expiryTimers.get(id);
  if (timer !== undefined) window.clearTimeout(timer);
  expiryTimers.delete(id);
}

function armExpiry(activity: LiveActivity): void {
  clearExpiry(activity.id);
  if (activity.mode !== "transient" || typeof activity.ttlMs !== "number") return;
  const timer = window.setTimeout(() => {
    expiryTimers.delete(activity.id);
    removeActivity(activity.id);
  }, Math.max(0, activity.ttlMs));
  expiryTimers.set(activity.id, timer);
}

function sameLeading(a: LiveLeading, b: LiveLeading): boolean {
  if (a.type !== b.type) return false;
  switch (a.type) {
    case "image":
      return a.url === (b as typeof a).url;
    case "initial": {
      const other = b as typeof a;
      return a.text === other.text && a.colors.join("|") === other.colors.join("|");
    }
    case "icon":
      return a.name === (b as typeof a).name && a.color === (b as typeof a).color;
    case "ring":
      return a.progress === (b as typeof a).progress;
    case "recordingDot":
      return a.color === (b as typeof a).color;
    case "icons": {
      const other = b as typeof a;
      return (
        a.icons.length === other.icons.length &&
        a.icons.every(
          (icon, index) =>
            icon.name === other.icons[index].name && icon.color === other.icons[index].color,
        )
      );
    }
    case "pulse":
      return a.name === (b as typeof a).name && a.color === (b as typeof a).color;
  }
}

function sameTrailing(a: LiveTrailing | undefined, b: LiveTrailing | undefined): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  if (typeof a === "string" || typeof b === "string") return false;
  return a.type === b.type && a.value === b.value && a.color === b.color;
}

function sameActions(a: LiveAction[] | undefined, b: LiveAction[] | undefined): boolean {
  if (!a || !b) return !a && !b;
  return (
    a.length === b.length &&
    a.every((action, index) => {
      const other = b[index];
      return action.id === other.id && action.label === other.label;
    })
  );
}

function sameActivity(a: LiveActivity, b: LiveActivity): boolean {
  return (
    a.id === b.id &&
    a.kind === b.kind &&
    a.mode === b.mode &&
    a.ttlMs === b.ttlMs &&
    a.priority === b.priority &&
    a.accent === b.accent &&
    a.label === b.label &&
    sameTrailing(a.trailing, b.trailing) &&
    a.playing === b.playing &&
    a.hover === b.hover &&
    a.maxWidth === b.maxWidth &&
    a.expandTab === b.expandTab &&
    a.render === b.render &&
    sameActions(a.actions, b.actions) &&
    sameLeading(a.leading, b.leading)
  );
}

/**
 * Adds an activity at the end of the start order, or replaces an existing one
 * in place (so its original start time still breaks priority ties). Transient
 * activities get a one-shot expiry timer; replacing one restarts its lifetime.
 */
export function registerActivity(activity: LiveActivity): void {
  const index = activities.findIndex((item) => item.id === activity.id);
  if (index < 0) {
    publish([...activities, activity]);
    armExpiry(activity);
    return;
  }
  if (sameActivity(activities[index], activity)) return;
  const next = activities.slice();
  next[index] = activity;
  publish(next);
  armExpiry(activity);
}

export function updateActivity(id: string, patch: Partial<LiveActivity>): void {
  const index = activities.findIndex((item) => item.id === id);
  if (index < 0) return;
  const previous = activities[index];
  const nextActivity = { ...previous, ...patch, id };
  if (sameActivity(previous, nextActivity)) return;
  const next = activities.slice();
  next[index] = nextActivity;
  publish(next);
  if (nextActivity.mode === "transient" && nextActivity.ttlMs !== previous.ttlMs) {
    armExpiry(nextActivity);
  }
}

export function removeActivity(id: string): void {
  clearExpiry(id);
  if (!activities.some((item) => item.id === id)) return;
  publish(activities.filter((item) => item.id !== id));
}

export function clearActivities(): void {
  for (const id of [...expiryTimers.keys()]) clearExpiry(id);
  if (activities.length) publish([]);
}

/**
 * Shared live-activity store. Register/update/remove are stable module
 * functions; `top` is the activity the compact bar should show right now.
 */
export function useLiveActivities(): {
  activities: readonly LiveActivity[];
  top: LiveActivity | null;
  register: typeof registerActivity;
  update: typeof updateActivity;
  remove: typeof removeActivity;
  clear: typeof clearActivities;
} {
  const list = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  return {
    activities: list,
    top: pickActivity(list),
    register: registerActivity,
    update: updateActivity,
    remove: removeActivity,
    clear: clearActivities,
  };
}
