import assert from "node:assert/strict";
import { test } from "node:test";
import { pickActivity } from "./pickActivity.ts";
import type { LiveActivity } from "./types.ts";

function activity(overrides: Partial<LiveActivity> = {}): LiveActivity {
  return {
    id: "music",
    kind: "music",
    mode: "persistent",
    priority: 10,
    label: "Song",
    leading: { type: "initial", text: "S", colors: ["#111111", "#222222"] },
    hover: "expand",
    ...overrides,
  };
}

test("no activities means no bar", () => {
  assert.equal(pickActivity([]), null);
});

test("any transient beats every persistent activity", () => {
  const persistent = activity({ id: "recording", kind: "recording", priority: 100 });
  const transient = activity({
    id: "meeting",
    kind: "meeting",
    mode: "transient",
    ttlMs: 6000,
    priority: 1,
  });

  assert.equal(pickActivity([persistent, transient]), transient);
  assert.equal(pickActivity([transient, persistent]), transient);
});

test("highest priority wins among transients", () => {
  const done = activity({
    id: "timer-done",
    kind: "timer-done",
    mode: "transient",
    ttlMs: 5000,
    priority: 90,
  });
  const meeting = activity({
    id: "meeting",
    kind: "meeting",
    mode: "transient",
    ttlMs: 6000,
    priority: 80,
  });

  assert.equal(pickActivity([meeting, done]), done);
  assert.equal(pickActivity([done, meeting]), done);
});

test("highest priority wins among persistent activities", () => {
  const music = activity({ id: "music", priority: 10 });
  const recording = activity({ id: "recording", kind: "recording", priority: 100 });
  const timer = activity({ id: "timer", kind: "timer", priority: 60 });

  assert.equal(pickActivity([music, timer, recording]), recording);
  assert.equal(pickActivity([recording, timer, music]), recording);
});

test("equal priorities use the most recently registered activity", () => {
  const first = activity({ id: "first", label: "First" });
  const second = activity({ id: "second", label: "Second" });
  const third = activity({ id: "third", label: "Third" });

  assert.equal(pickActivity([first, second, third]), third);
  assert.equal(pickActivity([third, first, second]), second);
});

test("an expired transient falls back to the best persistent activity", () => {
  const music = activity({ id: "music", priority: 10 });
  const recording = activity({ id: "recording", kind: "recording", priority: 100 });
  const meeting = activity({
    id: "meeting",
    kind: "meeting",
    mode: "transient",
    ttlMs: 6000,
    priority: 80,
  });
  const live = [music, recording, meeting];

  assert.equal(pickActivity(live), meeting);
  assert.equal(pickActivity(live.filter((item) => item.id !== "meeting")), recording);
  assert.equal(
    pickActivity(live.filter((item) => item.mode === "transient" && item.id !== "meeting")),
    null,
  );
});

test("picking does not mutate the list", () => {
  const list = [activity({ id: "music" }), activity({ id: "timer", priority: 60 })];
  const copy = [...list];
  pickActivity(list);
  assert.deepEqual(list, copy);
});
