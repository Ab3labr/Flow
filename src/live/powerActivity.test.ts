import assert from "node:assert/strict";
import { test } from "node:test";
import { powerActivity, reducePowerEvent } from "./powerActivity.ts";

function state(overrides: {
  transition?: "plugged" | "unplugged" | "none";
  pluggedIn?: boolean;
  charging?: boolean;
  percent?: number | null;
} = {}) {
  return {
    transition: overrides.transition ?? "none",
    pluggedIn: overrides.pluggedIn ?? false,
    charging: overrides.charging ?? false,
    percent: overrides.percent === undefined ? 50 : overrides.percent,
  } as const;
}

test("a plug transition registers even when nothing is visible", () => {
  assert.equal(reducePowerEvent(state({ transition: "plugged" }), false), "register");
});

test("an unplug transition registers even when a slot is visible", () => {
  assert.equal(reducePowerEvent(state({ transition: "unplugged" }), true), "register");
});

test("a percent change updates only a visible slot", () => {
  assert.equal(reducePowerEvent(state({ transition: "none" }), true), "update");
});

test("a drain from 81 to 80 on battery with nothing visible is ignored", () => {
  const before = state({ transition: "none", pluggedIn: false, percent: 81 });
  const after = state({ transition: "none", pluggedIn: false, percent: 80 });

  assert.equal(reducePowerEvent(before, false), "ignore");
  assert.equal(reducePowerEvent(after, false), "ignore");
});

test("an unknown percent shows the label alone", () => {
  const activity = powerActivity(state({ pluggedIn: false, percent: null }));
  assert.equal(activity.label, "Battery");
  assert.equal(activity.trailing, undefined);
});

test("a known percent is appended to the label", () => {
  const activity = powerActivity(state({ pluggedIn: false, percent: 80 }));
  assert.deepEqual(activity.trailing, {
    type: "text",
    value: "80%",
    color: "#30d158",
  });
});

test("plugged in below 100 says Charging and ignores the charging bit", () => {
  for (const charging of [false, true]) {
    const activity = powerActivity(
      state({ pluggedIn: true, charging, percent: 50 }),
    );
    assert.equal(activity.label, "Charging");
  }
});

test("plugged in at 100 says Charged and below 100 says Charging", () => {
  assert.equal(powerActivity(state({ pluggedIn: true, percent: 100 })).label, "Charged");
  assert.equal(powerActivity(state({ pluggedIn: true, percent: 99 })).label, "Charging");
});

test("unplugged says Battery regardless of the charging bit", () => {
  for (const charging of [false, true]) {
    const activity = powerActivity(
      state({ pluggedIn: false, charging, percent: 75 }),
    );
    assert.equal(activity.label, "Battery");
  }
});

test("low battery on unplugged power uses the red accent", () => {
  assert.equal(powerActivity(state({ pluggedIn: false, percent: 20 })).accent, "#ff453a");
  assert.equal(powerActivity(state({ pluggedIn: false, percent: 21 })).accent, "#30d158");
});
