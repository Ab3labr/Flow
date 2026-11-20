/**
 * Every Tauri event listener the app registers. `ui_ready` waits for these so
 * Rust never emits an open/close request into a page that is still attaching
 * its listeners (the startup notch-never-appears bug).
 */
const pending: Promise<unknown>[] = [];

export function registerTauriListener<T>(registration: Promise<T>): Promise<T> {
  pending.push(registration);
  return registration;
}

/** Resolves once every listener registered so far has settled. */
export async function whenListenersReady(): Promise<void> {
  // A listener registration may itself add another one (lazy services), so
  // drain in waves until nothing new arrives.
  while (pending.length > 0) {
    await Promise.allSettled(pending.splice(0));
  }
}
