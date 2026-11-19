import assert from "node:assert/strict";
import { test } from "node:test";
import { pickActivity } from "./pickActivity.ts";
import type { LiveActivity } from "./types.ts";

function activity(overrides: Partial<LiveActivity> = {}): LiveActivity {
  return {
    id: "music",
    kind: "music",
    priority: 10,
    label: "Song",
    leading: { type: "initial", text: "S", colors: ["#111111", "#222222"] },
    ...overrides,
  };
}

test("no activities means no bar", () => {
  assert.equal(pickActivity([]), null);
});

test("highest priority wins regardless of registration order", () => {
  const music = activity({ id: "music", priority: 10 });
  const recording = activity({ id: "recording", kind: "recording", priority: 100 });
  const timer = activity({ id: "timer", kind: "timer", priority: 90 });

  assert.equal(pickActivity([music, timer, recording]), recording);
  assert.equal(pickActivity([recording, timer, music]), recording);
});

test("equal priorities use the most recently started activity", () => {
  const first = activity({ id: "first", label: "First" });
  const second = activity({ id: "second", label: "Second" });
  const third = activity({ id: "third", label: "Third" });

  assert.equal(pickActivity([first, second, third]), third);
  assert.equal(pickActivity([third, first, second]), second);
});

test("picking does not mutate the list", () => {
  const list = [activity({ id: "music" }), activity({ id: "timer", priority: 90 })];
  const copy = [...list];
  pickActivity(list);
  assert.deepEqual(list, copy);
});
