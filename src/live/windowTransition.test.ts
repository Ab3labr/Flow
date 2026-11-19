import assert from "node:assert/strict";
import { test } from "node:test";
import {
  planKey,
  planWindow,
  retargetTransition,
  targetKey,
  transitionPhase,
  type WindowGeometry,
  type WindowTarget,
} from "./windowTransition.ts";

const geometry: WindowGeometry = { panelHeight: 172, barWidth: 180, barHeight: 30 };

function target(
  surface: WindowTarget["surface"],
  overrides: Partial<WindowGeometry> = {},
): WindowTarget {
  return { surface, geometry: { ...geometry, ...overrides } };
}

test("open to compact collapses while Rust holds the open window", () => {
  const from = target("open", { panelHeight: 250 });
  const to = target("compact", { barWidth: 180 });
  assert.equal(transitionPhase(from, to), "collapsing");

  const plan = planWindow({ from, to }, to, false);
  assert.equal(plan.mode, "open");
  assert.equal(plan.phase, "collapsing");
  assert.equal(plan.panelHeight, 250);
  assert.equal(plan.pinned, false);
});

test("compact to open expands into the target panel geometry", () => {
  const from = target("compact", { barWidth: 120 });
  const to = target("open", { panelHeight: 340 });
  assert.equal(transitionPhase(from, to), "expanding");

  const plan = planWindow({ from, to }, to, true);
  assert.equal(plan.mode, "open");
  assert.equal(plan.phase, "expanding");
  assert.equal(plan.panelHeight, 340);
  assert.equal(plan.pinned, true);
});

test("idle to compact grows before the bar appears", () => {
  const from = target("idle");
  const to = target("compact", { barWidth: 200 });
  const plan = planWindow({ from, to }, to, false);

  assert.equal(plan.mode, "pill");
  assert.equal(plan.phase, "expanding");
  assert.equal(plan.barWidth, 200);
});

test("compact width changes morph without switching window mode", () => {
  const from = target("compact", { barWidth: 260 });
  const to = target("compact", { barWidth: 170 });
  assert.equal(transitionPhase(from, to), "collapsing");

  const held = planWindow({ from, to }, to, false);
  assert.equal(held.mode, "pill");
  assert.equal(held.phase, "collapsing");
  assert.equal(held.barWidth, 260);

  const growing = planWindow({ from: to, to: from }, from, false);
  assert.equal(growing.phase, "expanding");
  assert.equal(growing.barWidth, 260);
});

test("identical geometry needs no transition", () => {
  const current = target("compact");
  assert.equal(transitionPhase(current, target("compact")), null);
  assert.equal(targetKey(current), targetKey(target("compact")));

  const plan = planWindow(null, current, false);
  assert.equal(plan.phase, "stable");
  assert.equal(plan.mode, "pill");
});

test("plan keys ignore dimensions Rust does not use", () => {
  const openA = planWindow(null, target("open", { barWidth: 120 }), false);
  const openB = planWindow(null, target("open", { barWidth: 260 }), false);
  assert.equal(planKey(openA), planKey(openB));

  const pinnedA = planWindow(null, target("compact"), false);
  const pinnedB = planWindow(null, target("compact"), true);
  assert.notEqual(planKey(pinnedA), planKey(pinnedB));
});

test("reverse direction keeps the original source until settling", () => {
  const source = target("open", { panelHeight: 250 });
  const compact = target("compact", { barWidth: 200 });
  const reversed = planWindow({ from: source, to: compact }, source, false);

  assert.equal(reversed.mode, "open");
  assert.equal(reversed.phase, "collapsing");
});

test("retargeting flips the source when a morph reverses", () => {
  const open = target("open", { panelHeight: 250 });
  const compact = target("compact", { barWidth: 200 });
  const idle = target("idle");

  assert.deepEqual(retargetTransition({ from: open, to: compact }, open), {
    from: compact,
    to: open,
  });
  assert.deepEqual(retargetTransition({ from: idle, to: compact }, idle), {
    from: compact,
    to: idle,
  });
  assert.deepEqual(retargetTransition({ from: idle, to: compact }, open), {
    from: idle,
    to: open,
  });
});
