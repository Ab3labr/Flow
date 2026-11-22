import type { NotchSettings } from "./types";

export type ShortcutField = "hotkeyNotes" | "hotkeyNext" | "hotkeyPrev";

export const SHORTCUT_FIELDS: ShortcutField[] = [
  "hotkeyNotes",
  "hotkeyNext",
  "hotkeyPrev",
];

export type ParsedShortcut = {
  modifiers: string[];
  key: string;
  canonical: string;
};

const MODIFIER_ORDER = ["Ctrl", "Alt", "Shift", "Meta"] as const;
const MODIFIER_ALIASES: Record<string, string> = {
  CTRL: "Ctrl",
  CONTROL: "Ctrl",
  ALT: "Alt",
  SHIFT: "Shift",
  META: "Meta",
  WIN: "Meta",
  WINDOWS: "Meta",
  SUPER: "Meta",
};

function keyToken(token: string): string | null {
  if (/^[A-Z]$/.test(token) || /^[0-9]$/.test(token)) return token;
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(token)) return token;
  if (
    [
      "BACKSPACE",
      "TAB",
      "ENTER",
      "ESC",
      "ESCAPE",
      "SPACE",
      "PAGEUP",
      "PAGEDOWN",
      "END",
      "HOME",
      "LEFT",
      "UP",
      "RIGHT",
      "DOWN",
      "INSERT",
      "DELETE",
    ].includes(token)
  ) {
    return token;
  }
  return null;
}

/** Parses `Alt+N` or `Ctrl + Alt + F`. A modifier and a key are both required. */
export function parseShortcut(input: string): ParsedShortcut | null {
  const modifiers = new Set<string>();
  let key: string | null = null;
  for (const raw of input.split("+")) {
    const token = raw.trim().toUpperCase();
    if (!token) return null;
    const modifier = MODIFIER_ALIASES[token];
    if (modifier) {
      modifiers.add(modifier);
      continue;
    }
    if (key) return null;
    key = keyToken(token);
    if (!key) return null;
  }
  if (modifiers.size === 0 || !key) return null;
  const ordered = MODIFIER_ORDER.filter((name) => modifiers.has(name));
  const canonical = [...ordered, key].join(" + ");
  return { modifiers: ordered.slice(), key, canonical };
}

/** Same canonical form as `parseShortcut`, but allows the double-Alt literal. */
export function normalizeShortcut(input: string): string {
  return input
    .split("+")
    .map((part) => part.trim().toUpperCase())
    .filter(Boolean)
    .join(" ");
}

/** Formats a captured `keydown` into the stored `Ctrl + Alt + N` style. */
export function formatPressedKey(
  key: string,
  modifiers: { ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean },
): string {
  const pressedKey =
    key === " " ? "Space" : key.length === 1 ? key.toUpperCase() : key.replace("Control", "Ctrl");
  const parts = [
    modifiers.ctrlKey && pressedKey !== "Ctrl" ? "Ctrl" : "",
    modifiers.altKey && pressedKey !== "Alt" ? "Alt" : "",
    modifiers.shiftKey && pressedKey !== "Shift" ? "Shift" : "",
    modifiers.metaKey && pressedKey !== "Meta" ? "Meta" : "",
    pressedKey,
  ].filter(Boolean);
  return parts.join(" + ");
}

/** Validates the three global bindings against each other and double-Alt. */
export function validateHotkeys(
  settings: Pick<NotchSettings, ShortcutField | "shortcut">,
): Record<ShortcutField, string | null> {
  const errors: Record<ShortcutField, string | null> = {
    hotkeyNotes: null,
    hotkeyNext: null,
    hotkeyPrev: null,
  };
  const seen = new Map<string, ShortcutField>();
  const doubleAlt = normalizeShortcut(settings.shortcut);

  for (const field of SHORTCUT_FIELDS) {
    const parsed = parseShortcut(settings[field]);
    if (!parsed) {
      errors[field] = "Needs a modifier and a key";
      continue;
    }
    const canonical = normalizeShortcut(parsed.canonical);
    if (canonical === doubleAlt || seen.has(canonical)) {
      errors[field] = "Shortcut already used";
      continue;
    }
    seen.set(canonical, field);
  }
  return errors;
}

/** True when a newly captured double-Alt/custom shortcut collides with a hotkey. */
export function shortcutConflictsWithHotkeys(
  candidate: string,
  settings: Pick<NotchSettings, ShortcutField | "shortcut">,
): boolean {
  const value = normalizeShortcut(candidate);
  return SHORTCUT_FIELDS.some(
    (field) => normalizeShortcut(settings[field]) === value,
  );
}
