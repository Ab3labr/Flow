import assert from "node:assert/strict";
import { test } from "node:test";
import { privacyActivity } from "./privacyActivity.ts";
import { LIVE_PRIORITIES } from "./priorities.ts";

test("microphone start uses the orange mic icon and label", () => {
  const activity = privacyActivity({ kinds: ["microphone"], app: "Chrome" });
  assert.equal(activity.label, "Microphone");
  assert.deepEqual(activity.leading, { type: "icon", name: "mic", color: "#ff9f0a" });
  assert.deepEqual(activity.trailing, { type: "text", value: "Chrome" });
});

test("camera start uses the green camera icon and label", () => {
  const activity = privacyActivity({ kinds: ["camera"], app: "Teams" });
  assert.equal(activity.label, "Camera");
  assert.deepEqual(activity.leading, { type: "icon", name: "camera", color: "#30d158" });
});

test("combined start shows both icons and one label", () => {
  const activity = privacyActivity({ kinds: ["microphone", "camera"], app: "Teams" });
  assert.equal(activity.label, "Camera and mic");
  assert.deepEqual(activity.leading, {
    type: "icons",
    icons: [
      { name: "mic", color: "#ff9f0a" },
      { name: "camera", color: "#30d158" },
    ],
  });
});

test("privacy activity is a 4 s non-hover slot above charging", () => {
  const activity = privacyActivity({ kinds: ["microphone"], app: "Chrome" });
  assert.equal(activity.mode, "transient");
  assert.equal(activity.ttlMs, 4_000);
  assert.equal(activity.hover, "none");
  assert.equal(activity.priority, 97);
  assert.ok(activity.priority > LIVE_PRIORITIES.charging);
  assert.equal(activity.maxWidth, 260);
});
