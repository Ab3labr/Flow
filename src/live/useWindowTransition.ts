import { useEffect, useRef, useState } from "react";
import { setUiMode } from "../services/uiService";
import {
  planWindow,
  retargetTransition,
  targetKey,
  type WindowTarget,
  type WindowTransition,
} from "./windowTransition";

/** Spring 380/30/0.9 settles well inside this; Rust resizes only once after it. */
export const TRANSITION_SETTLE_MS = 320;
export const REDUCED_TRANSITION_SETTLE_MS = 160;

export function useWindowTransition(
  target: WindowTarget,
  pinned: boolean,
  reducedMotion: boolean,
): { transitioning: boolean } {
  const [transition, setTransition] = useState<WindowTransition | null>(null);
  const settled = useRef<WindowTarget>(target);
  const key = targetKey(target);

  // Start a morph on the first geometry change; retarget it if the user
  // reverses direction instead of jumping straight to the new window size.
  useEffect(() => {
    setTransition((current) => {
      if (!current) {
        if (key === targetKey(settled.current)) return current;
        return { from: settled.current, to: target };
      }
      return retargetTransition(current, target);
    });
  }, [key, target]);

  // Use the pending transition during this render so Rust never receives a
  // stable plan for a frame where the visual state is already changing.
  const effectiveTransition: WindowTransition | null =
    transition ?? (key === targetKey(settled.current) ? null : { from: settled.current, to: target });
  const fromKey = effectiveTransition ? targetKey(effectiveTransition.from) : null;
  const toKey = effectiveTransition ? targetKey(effectiveTransition.to) : null;

  // A single final resize per morph: grow before expanding, shrink after.
  useEffect(() => {
    if (!transition) return;
    const id = window.setTimeout(
      () => {
        settled.current = transition.to;
        setTransition(null);
      },
      reducedMotion ? REDUCED_TRANSITION_SETTLE_MS : TRANSITION_SETTLE_MS,
    );
    return () => window.clearTimeout(id);
  }, [fromKey, toKey, reducedMotion, transition]);

  const plan = planWindow(effectiveTransition, target, pinned);
  useEffect(() => {
    setUiMode(plan);
  }, [plan.barHeight, plan.barWidth, plan.mode, plan.panelHeight, plan.pinned, plan.phase]);

  return { transitioning: effectiveTransition !== null };
}
