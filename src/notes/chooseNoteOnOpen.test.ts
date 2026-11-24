import assert from "node:assert/strict";
import { test } from "node:test";
import { chooseNoteOnOpen } from "./chooseNoteOnOpen.ts";

const MINUTE = 60_000;
const NOW = 1_800_000_000_000;
const ID = "2026-11-24 10-00-00 Note.md";

test("recent edit reopens the same note", () => {
  assert.equal(
    chooseNoteOnOpen(NOW, NOW - 5 * MINUTE, ID, true, 720, false),
    ID,
  );
});

test("old edit opens a fresh draft", () => {
  assert.equal(
    chooseNoteOnOpen(NOW, NOW - 13 * 60 * MINUTE, ID, true, 720, false),
    null,
  );
});

test("a missing file opens a fresh draft", () => {
  assert.equal(
    chooseNoteOnOpen(NOW, NOW - MINUTE, ID, false, 720, false),
    null,
  );
});

test("newAfterMinutes 0 never expires the last note", () => {
  assert.equal(
    chooseNoteOnOpen(NOW, NOW - 365 * 24 * 60 * MINUTE, ID, true, 0, false),
    ID,
  );
});

test("New note pressed always opens a fresh draft", () => {
  assert.equal(
    chooseNoteOnOpen(NOW, NOW - MINUTE, ID, true, 720, true),
    null,
  );
});

test("no remembered id opens a fresh draft", () => {
  assert.equal(chooseNoteOnOpen(NOW, NOW - MINUTE, null, true, 720, false), null);
});
