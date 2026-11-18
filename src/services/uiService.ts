import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriApp } from "./env";

export type UiMode = "idle" | "pill" | "open";

export type OpenRequest = { tab?: string; source?: "hover" };

export function setUiMode(mode: UiMode, panelHeight: number): void {
  if (!isTauriApp) return;
  void invoke("set_ui_mode", { mode, panelHeight }).catch((error) => {
    console.error("set_ui_mode failed", error);
  });
}

export function onOpenRequest(handler: (request: OpenRequest) => void): Promise<() => void> {
  if (!isTauriApp) return Promise.resolve(() => {});
  return listen<OpenRequest | null>("open-request", (event) => {
    handler(event.payload ?? {});
  });
}

/** Rust double-Alt hook: toggle the panel open (pinned) or closed. */
export function onToggleRequest(handler: () => void): Promise<() => void> {
  if (!isTauriApp) return Promise.resolve(() => {});
  return listen<{ source?: string } | null>("toggle-request", () => {
    handler();
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
