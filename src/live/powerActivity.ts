import { LIVE_PRIORITIES } from "./priorities.ts";
import type { PowerState } from "../services/types.ts";
import type { LiveActivity } from "./types.ts";

/** What a `power-state` event should do to the compact bar. */
export type PowerEventAction = "register" | "update" | "ignore";

/**
 * Decide how a power event affects the bar. A real plug/unplug transition
 * (re)starts the 4 s slot. A percent-only update refreshes a slot that is
 * already visible and is ignored otherwise, so a 1% drain never pops the bar
 * up on its own.
 */
export function reducePowerEvent(
  state: Pick<PowerState, "transition">,
  visible: boolean,
): PowerEventAction {
  if (state.transition === "plugged" || state.transition === "unplugged") {
    return "register";
  }
  return visible ? "update" : "ignore";
}

/**
 * Label comes from the AC state and percent, never from the BatteryFlag
 * charging bit (which Windows can lag or leave unset right after plugging in).
 */
function powerLabel(state: PowerState): string {
  if (!state.pluggedIn) return "Battery";
  return state.percent != null && state.percent >= 100 ? "Charged" : "Charging";
}

/** A `power-state` event becomes a 4 s transient compact-bar slot. */
export function powerActivity(state: PowerState): LiveActivity {
  const low = !state.pluggedIn && state.percent != null && state.percent <= 20;
  const color = low ? "#ff453a" : "#30d158";
  return {
    id: "charging",
    kind: "charging",
    mode: "transient",
    ttlMs: 4_000,
    priority: LIVE_PRIORITIES.charging,
    accent: color,
    leading: {
      type: "icon",
      name: state.pluggedIn ? "charging" : "battery",
      color,
    },
    label: powerLabel(state),
    // Unknown percent: label alone; add the number once it is known.
    trailing:
      state.percent == null
        ? undefined
        : { type: "text", value: `${state.percent}%`, color },
    hover: "none",
    maxWidth: 200,
  };
}
