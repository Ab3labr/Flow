import assert from "node:assert/strict";
import { test } from "node:test";
import { registerTauriListener, whenListenersReady } from "./listenerRegistry.ts";

test("ui_ready waits for every listener registered so far", async () => {
  let resolved = false;
  const registration = new Promise<void>((resolve) => {
    setTimeout(() => {
      resolved = true;
      resolve();
    }, 10);
  });
  registerTauriListener(registration);

  await whenListenersReady();
  assert.equal(resolved, true);
});

test("listeners registered while draining are awaited too", async () => {
  const order: string[] = [];
  const first = Promise.resolve().then(() => {
    order.push("first");
    // A lazy service may attach its listener from an earlier await.
    registerTauriListener(Promise.resolve().then(() => order.push("second")));
  });
  registerTauriListener(first);

  await whenListenersReady();
  assert.deepEqual(order, ["first", "second"]);
});

test("an empty registry resolves immediately", async () => {
  await whenListenersReady();
});
