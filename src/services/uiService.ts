import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriApp } from "./env";
import { planKey, type WindowPlan } from "../live/windowTransition";

export type UiMode = "idle" | "pill" | "open";
export type UiPhase = "stable" | "expanding" | "collapsing";

export type OpenRequest = {
  tab?: string;
  source?: "hover" | "alt" | "tray";
  pinned?: boolean;
};

export type CloseRequest = { source?: "alt" | "esc" | "outside" };

/**
 * One command owns all window geometry. The plan already carries the mode
 * Rust should apply right now (source during a collapse, target during an
 * expansion) and the phase that decides input/visibility.
 */
let lastPlanKey: string | null = null;

export function setUiMode(plan: WindowPlan): void {
  if (!isTauriApp) return;
  const key = planKey(plan);
  if (key === lastPlanKey) return;
  lastPlanKey = key;
  void invoke("set_ui_mode", {
    mode: plan.mode,
    phase: plan.phase,
    panelHeight: plan.panelHeight,
    pinned: plan.pinned,
    barWidth: plan.barWidth,
    barHeight: plan.barHeight,
  }).catch((error) => {
    // Allow a retry if the command failed; Rust also dedupes at the API layer.
    lastPlanKey = null;
    console.error("set_ui_mode failed", error);
  });
}

/**
 * Sends a webview log line through tauri-plugin-log's existing `log` command
 * (levels: 4 = warn, 5 = error). Falls back to the browser console in mock
 * mode or if the command is unavailable.
 */
export function logWebview(message: string, level: 4 | 5 = 4): void {
  if (!isTauriApp) {
    console.warn(message);
    return;
  }
  void invoke("plugin:log|log", { level, message }).catch(() => {
    console.warn(message);
  });
}

/** True while a fullscreen app is in front (Rust trigger probe). */
export function isFullscreenAppActive(): Promise<boolean> {
  if (!isTauriApp) return Promise.resolve(false);
  return invoke<boolean>("fullscreen_active").catch(() => false);
}

export function onOpenRequest(handler: (request: OpenRequest) => void): Promise<() => void> {
  if (!isTauriApp) return Promise.resolve(() => {});
  return listen<OpenRequest | null>("open-request", (event) => {
    handler(event.payload ?? {});
  });
}

/** Rust decisions: close on double-Alt, Esc, or an outside click. */
export function onCloseRequest(handler: (request: CloseRequest) => void): Promise<() => void> {
  if (!isTauriApp) return Promise.resolve(() => {});
  return listen<CloseRequest | null>("close-request", (event) => {
    handler(event.payload ?? {});
  });
}

/** Focuses the notch window once Rust has shown it. */
export function focusNotchWindow(): void {
  if (!isTauriApp) return;
  void getCurrentWindow()
    .setFocus()
    .catch((error) => {
      console.error("setFocus failed", error);
    });
}
