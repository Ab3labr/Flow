import type { ReactNode } from "react";
import type { TabId } from "../components/TopBar";

export type LiveLeading =
  | { type: "image"; url: string }
  | { type: "initial"; text: string; colors: string[] }
  | { type: "icon"; name: string; color?: string }
  | { type: "ring"; progress: number }
  | { type: "recordingDot"; color: string }
  | { type: "pulse"; name: string; color?: string };

export type LiveTrailing =
  | "equalizer"
  | "dot"
  | "time"
  | "pause"
  | "none"
  | { type: "text"; value: string; color?: string };

export type LiveAction = {
  id: string;
  icon: ReactNode;
  label: string;
  onClick: () => void;
};

export type LiveRenderContext = {
  activity: LiveActivity;
  playing: boolean;
  color: string;
  maxWidth: number;
};

/**
 * One activity the compact bar can show. Activities are registered in start
 * order; `pickActivity` uses that order to break priority ties. Transient
 * activities carry a `ttlMs` and are removed by the store when they expire.
 */
export interface LiveActivity {
  id: string;
  kind: "music" | "recording" | "timer" | "timer-done" | "meeting" | string;
  mode: "persistent" | "transient";
  /** Transient only: lifetime before the bar falls back to a persistent slot. */
  ttlMs?: number;
  priority: number;
  accent?: string;
  leading: LiveLeading;
  label: string;
  trailing?: LiveTrailing;
  playing?: boolean;
  hover: "expand" | "actions" | "none";
  actions?: LiveAction[];
  expandTab?: TabId;
  maxWidth?: number;
  /** Optional full-slot renderer for future wide/custom activities. */
  render?: (ctx: LiveRenderContext) => ReactNode;
}
