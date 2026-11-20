import type { UiMode, UiPhase } from "../services/uiService.ts";
import { IDLE_BAR_HEIGHT, IDLE_BAR_WIDTH } from "./geometry.ts";

/** The one visual container's settled state. */
export type Surface = "idle" | "compact" | "open";

export type WindowGeometry = {
  panelHeight: number;
  barWidth: number;
  barHeight: number;
};

export type WindowTarget = {
  surface: Surface;
  geometry: WindowGeometry;
};

/** An in-flight morph. `from` is held by Rust until the animation settles. */
export type WindowTransition = {
  from: WindowTarget;
  to: WindowTarget;
};

export type WindowPlan = {
  /** Physical mode Rust should apply right now (source while collapsing). */
  mode: UiMode;
  phase: UiPhase;
  panelHeight: number;
  barWidth: number;
  barHeight: number;
  pinned: boolean;
  /** Logical panel state, from the target surface. Triggers use this, so a
   * quick double-Alt during a collapse reopens instead of closing again. */
  open: boolean;
};

const SURFACE_RANK: Record<Surface, number> = { idle: 0, compact: 1, open: 2 };

function modeFor(surface: Surface): UiMode {
  // The Rust mode keeps its historical name for backward compatibility.
  if (surface === "compact") return "pill";
  return surface;
}

function metric(target: WindowTarget): number {
  if (target.surface === "open") return target.geometry.panelHeight;
  if (target.surface === "compact") return target.geometry.barWidth;
  return 0;
}

/** `null` means the two targets are visually identical. */
export function transitionPhase(from: WindowTarget, to: WindowTarget): UiPhase | null {
  const rankDelta = SURFACE_RANK[to.surface] - SURFACE_RANK[from.surface];
  if (rankDelta > 0) return "expanding";
  if (rankDelta < 0) return "collapsing";
  const delta = metric(to) - metric(from);
  if (delta > 0.01) return "expanding";
  if (delta < -0.01) return "collapsing";
  return null;
}

/**
 * When the user reverses a morph, the source becomes the geometry the visual
 * container was heading toward. Continuing in the same direction keeps the
 * original source so Rust never resizes mid-gesture.
 */
export function retargetTransition(
  current: WindowTransition,
  target: WindowTarget,
): WindowTransition {
  if (targetKey(target) === targetKey(current.to)) return current;
  const phase = transitionPhase(current.from, current.to);
  const nextPhase = transitionPhase(current.from, target);
  const reversed = nextPhase === null || (phase !== null && nextPhase !== phase);
  return { from: reversed ? current.to : current.from, to: target };
}

/** Stable identity for a target; pinned and ignored dimensions are excluded. */
export function targetKey(target: WindowTarget): string {
  switch (target.surface) {
    case "idle":
      return "idle";
    case "compact":
      return `compact:${Math.round(target.geometry.barWidth)}:${Math.round(
        target.geometry.barHeight,
      )}`;
    case "open":
      return `open:${Math.round(target.geometry.panelHeight * 100)}`;
  }
}

function geometryFor(target: WindowTarget): WindowGeometry {
  if (target.surface === "idle") {
    return {
      panelHeight: target.geometry.panelHeight,
      barWidth: IDLE_BAR_WIDTH,
      barHeight: IDLE_BAR_HEIGHT,
    };
  }
  return target.geometry;
}

/**
 * Rust should grow before expanding and shrink after collapsing. During a
 * transition it keeps the source mode/geometry and ignores cursor input, so
 * the large transparent window never captures clicks while Morph runs.
 */
export function planWindow(
  transition: WindowTransition | null,
  target: WindowTarget,
  pinned: boolean,
): WindowPlan {
  if (!transition) {
    const geometry = geometryFor(target);
    return {
      mode: modeFor(target.surface),
      phase: "stable",
      ...geometry,
      pinned,
      open: target.surface === "open",
    };
  }

  const phase = transitionPhase(transition.from, transition.to) ?? "stable";
  const source = phase === "collapsing" ? transition.from : transition.to;
  const geometry = geometryFor(source);
  return {
    mode: modeFor(source.surface),
    phase,
    ...geometry,
    pinned: source.surface === "open" ? pinned : false,
    open: target.surface === "open",
  };
}

/**
 * Arguments for the `set_ui_mode` command. Tauri renames Rust parameters to
 * camelCase, so the optional `logical_open` flag must be sent as
 * `logicalOpen`; sending `open` silently fell back to `mode == "open"` and
 * made Rust believe a collapsing panel was still open.
 */
export function uiModeArgs(plan: WindowPlan) {
  return {
    mode: plan.mode,
    phase: plan.phase,
    panelHeight: plan.panelHeight,
    pinned: plan.pinned,
    barWidth: plan.barWidth,
    barHeight: plan.barHeight,
    logicalOpen: plan.open,
  };
}

/** IPC commands are skipped when the effective window state is unchanged. */
export function planKey(plan: WindowPlan): string {
  const dimensions =
    plan.mode === "open"
      ? `h${Math.round(plan.panelHeight * 100)}`
      : plan.mode === "pill"
        ? `w${Math.round(plan.barWidth)}h${Math.round(plan.barHeight)}`
        : "";
  return `${plan.mode}|${plan.phase}|${plan.open}|${plan.pinned}|${dimensions}`;
}
