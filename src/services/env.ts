/** True inside the Tauri shell; false in the browser prototype. */
export const isTauriApp = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Mock data/services: explicit VITE_USE_MOCK=1, or plain browser dev. */
export const isMockMode = import.meta.env.VITE_USE_MOCK === "1" || !isTauriApp;
