import type { TabId } from "../components/TopBar";

export type LiveLeading =
  | { type: "image"; url: string }
  | { type: "initial"; text: string; colors: string[] }
  | { type: "icon"; name: string };

export type LiveTrailing = "equalizer" | "dot" | "time" | "none";

/**
 * One activity the compact bar can show. Activities are registered in start
 * order; `pickActivity` uses that order to break priority ties.
 */
export interface LiveActivity {
  id: string;
  kind: "music" | "recording" | "timer" | "meeting" | string;
  priority: number;
  accent?: string;
  leading: LiveLeading;
  label: string;
  trailing?: LiveTrailing;
  playing?: boolean;
  expandTab?: TabId;
}
