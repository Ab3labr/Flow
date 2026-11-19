import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriApp } from "./env";

export type UiMode = "idle" | "pill" | "open";

export type OpenRequest = {
  tab?: string;
  source?: "hover" | "alt" | "tray";
  pinned?: boolean;
};

export type CloseRequest = { source?: "alt" | "esc" | "outside" };

export function setUiMode(
  mode: UiMode,
  panelHeight: number,
  pinned = false,
  barWidth = 108,
  barHeight = 26,
): void {
  if (!isTauriApp) return;
  void invoke("set_ui_mode", {
    mode,
    panelHeight,
    pinned,
    barWidth,
    barHeight,
  }).catch((error) => {
    console.error("set_ui_mode failed", error);
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
