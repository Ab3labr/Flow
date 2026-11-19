import { useSyncExternalStore } from "react";
import { pickActivity } from "./pickActivity";
import type { LiveActivity, LiveLeading } from "./types";

let activities: LiveActivity[] = [];
let snapshot: readonly LiveActivity[] = activities;
const listeners = new Set<() => void>();

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

function sameLeading(a: LiveLeading, b: LiveLeading): boolean {
  if (a.type !== b.type) return false;
  if (a.type === "image" && b.type === "image") return a.url === b.url;
  if (a.type === "initial" && b.type === "initial") {
    return a.text === b.text && a.colors.join("|") === b.colors.join("|");
  }
  if (a.type === "icon" && b.type === "icon") return a.name === b.name;
  return false;
}

function sameActivity(a: LiveActivity, b: LiveActivity): boolean {
  return (
    a.id === b.id &&
    a.kind === b.kind &&
    a.priority === b.priority &&
    a.accent === b.accent &&
    a.label === b.label &&
    a.trailing === b.trailing &&
    a.playing === b.playing &&
    a.expandTab === b.expandTab &&
    sameLeading(a.leading, b.leading)
  );
}

/**
 * Adds an activity at the end of the start order, or replaces an existing one
 * in place (so its original start time still breaks priority ties).
 */
export function registerActivity(activity: LiveActivity): void {
  const index = activities.findIndex((item) => item.id === activity.id);
  if (index < 0) {
    publish([...activities, activity]);
    return;
  }
  if (sameActivity(activities[index], activity)) return;
  const next = activities.slice();
  next[index] = activity;
  publish(next);
}

export function updateActivity(id: string, patch: Partial<LiveActivity>): void {
  const index = activities.findIndex((item) => item.id === id);
  if (index < 0) return;
  const nextActivity = { ...activities[index], ...patch, id };
  if (sameActivity(activities[index], nextActivity)) return;
  const next = activities.slice();
  next[index] = nextActivity;
  publish(next);
}

export function removeActivity(id: string): void {
  if (!activities.some((item) => item.id === id)) return;
  publish(activities.filter((item) => item.id !== id));
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
} {
  const list = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  return {
    activities: list,
    top: pickActivity(list),
    register: registerActivity,
    update: updateActivity,
    remove: removeActivity,
  };
}
