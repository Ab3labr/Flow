/** True inside the Tauri shell; false in the browser prototype. */
export const isTauriApp = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
