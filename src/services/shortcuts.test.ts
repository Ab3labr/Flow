import assert from "node:assert/strict";
import { test } from "node:test";
import {
  formatPressedKey,
  parseShortcut,
  shortcutConflictsWithHotkeys,
  validateHotkeys,
} from "./shortcuts.ts";
import { DEFAULT_NOTCH_SETTINGS } from "./types.ts";

test("global shortcut defaults parse with at least one modifier", () => {
  assert.equal(parseShortcut("Alt+N")?.canonical, "Alt + N");
  assert.equal(parseShortcut("Alt+C")?.key, "C");
  assert.equal(parseShortcut("Alt+Z")?.key, "Z");
  assert.equal(parseShortcut("N"), null);
  assert.equal(parseShortcut("Alt"), null);
  assert.equal(parseShortcut("Alt Alt"), null);
});

test("captured keydown formats into the stored canonical style", () => {
  assert.equal(formatPressedKey("n", { ctrlKey: false, altKey: true, shiftKey: false, metaKey: false }), "Alt + N");
  assert.equal(
    formatPressedKey("Control", { ctrlKey: true, altKey: false, shiftKey: false, metaKey: false }),
    "Ctrl",
  );
});

test("three global bindings reject duplicates and the double-Alt trigger", () => {
  const base = { ...DEFAULT_NOTCH_SETTINGS };
  assert.deepEqual(validateHotkeys(base), {
    hotkeyNotes: null,
    hotkeyNext: null,
    hotkeyPrev: null,
  });

  assert.equal(validateHotkeys({ ...base, hotkeyNext: "Alt+N" }).hotkeyNext, "Shortcut already used");
  assert.equal(
    validateHotkeys({ ...base, hotkeyPrev: "Alt + C" }).hotkeyPrev,
    "Shortcut already used",
  );
  assert.equal(
    validateHotkeys({ ...base, shortcut: "Alt + Z" }).hotkeyPrev,
    "Shortcut already used",
  );
  assert.equal(
    validateHotkeys({ ...base, hotkeyNotes: "N" }).hotkeyNotes,
    "Needs a modifier and a key",
  );
});

test("a new double-Alt capture cannot collide with a global binding", () => {
  assert.equal(shortcutConflictsWithHotkeys("Alt+Z", DEFAULT_NOTCH_SETTINGS), true);
  assert.equal(shortcutConflictsWithHotkeys("Alt Alt", DEFAULT_NOTCH_SETTINGS), false);
});
