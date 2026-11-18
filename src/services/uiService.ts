import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauriApp } from "./env";

export type UiMode = "idle" | "pill" | "open";

export function setUiMode(mode: UiMode, panelHeight: number): void {
  if (!isTauriApp) return;
  void invoke("set_ui_mode", { mode, panelHeight }).catch((error) => {
    console.error("set_ui_mode failed", error);
  });
}

export function onOpenRequest(handler: (tab?: string) => void): Promise<() => void> {
  if (!isTauriApp) return Promise.resolve(() => {});
  return listen<{ tab?: string } | null>("open-request", (event) => {
    handler(event.payload?.tab);
  });
}
