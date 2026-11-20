import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriApp } from "./env";
import { debugLog, setDebugEnabled, setDebugSink } from "./debugLog";
import { registerTauriListener, whenListenersReady } from "./listenerRegistry";
import { planKey, uiModeArgs, type WindowPlan } from "../live/windowTransition";

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

export function setUiMode(plan: WindowPlan, force = false): void {
  if (!isTauriApp) return;
  const key = planKey(plan);
  if (!force && key === lastPlanKey) {
    debugLog(`[ui] setUiMode deduped ${key}`);
    return;
  }
  lastPlanKey = key;
  debugLog(`[ui] setUiMode ${force ? "forced" : "sent"} ${key}`);
  void invoke("set_ui_mode", uiModeArgs(plan)).catch((error) => {
    // Allow a retry if the command failed; Rust also dedupes at the API layer.
    lastPlanKey = null;
    console.error("set_ui_mode failed", error);
  });
}

/**
 * Startup handshake: wait for every listener, then let Rust reset itself to
 * idle/stable/open=false and release any queued trigger. The caller re-sends
 * the first plan afterwards with `setUiMode(plan, true)` so the two sides are
 * guaranteed to agree even when that plan equals Rust's default.
 */
let readyPromise: Promise<void> | null = null;

export function uiReady(): Promise<void> {
  if (!isTauriApp) return Promise.resolve();
  if (!readyPromise) {
    readyPromise = (async () => {
      await whenListenersReady();
      debugLog(`[ui] all Tauri listeners registered visibility=${document.visibilityState}`);
      // The first plan after the handshake must not be dropped.
      lastPlanKey = null;
      const debug = await invoke<boolean>("ui_ready");
      setDebugEnabled(debug);
      debugLog("[ui] ui_ready complete (idle/stable/logical_open=false)");
    })();
  }
  return readyPromise;
}

/**
 * Sends a webview log line through tauri-plugin-log's existing `log` command
 * (levels: 3 = info, 4 = warn, 5 = error). Falls back to the browser console
 * in mock mode or if the command is unavailable.
 */
export function logWebview(message: string, level: 3 | 4 | 5 = 4): void {
  if (!isTauriApp) {
    console.warn(message);
    return;
  }
  void invoke("plugin:log|log", { level, message }).catch(() => {
    console.warn(message);
  });
}

// Debug stage lines also land in float.log via the plugin.
setDebugSink((line) => logWebview(line, 3));
debugLog(`[ui] webview module start visibility=${document.visibilityState}`);

/** True while a fullscreen app is in front (Rust trigger probe). */
export function isFullscreenAppActive(): Promise<boolean> {
  if (!isTauriApp) return Promise.resolve(false);
  return invoke<boolean>("fullscreen_active").catch(() => false);
}

export function onOpenRequest(handler: (request: OpenRequest) => void): Promise<() => void> {
  if (!isTauriApp) return Promise.resolve(() => {});
  const registration = registerTauriListener(
    listen<OpenRequest | null>("open-request", (event) => {
      debugLog(`[ui] open-request received source=${event.payload?.source ?? "?"}`);
      handler(event.payload ?? {});
    }),
  );
  void registration.then(() => debugLog("[ui] listener registered: open-request"));
  return registration;
}

/** Rust decisions: close on double-Alt, Esc, or an outside click. */
export function onCloseRequest(handler: (request: CloseRequest) => void): Promise<() => void> {
  if (!isTauriApp) return Promise.resolve(() => {});
  const registration = registerTauriListener(
    listen<CloseRequest | null>("close-request", (event) => {
      debugLog(`[ui] close-request received source=${event.payload?.source ?? "?"}`);
      handler(event.payload ?? {});
    }),
  );
  void registration.then(() => debugLog("[ui] listener registered: close-request"));
  return registration;
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
