import { useCallback, useEffect, useState } from "react";
import { timerService, type TimerSnapshot, type TimerStatus } from "../services";

export type TimerController = {
  state: TimerStatus;
  totalMs: number;
  /** Locally interpolated every 250 ms while running. */
  remainingMs: number;
  endsAtMs?: number;
  lastCustomMs: number | null;
  ready: boolean;
  setDuration: (durationMs: number) => void;
  start: () => void;
  pause: () => void;
  resume: () => void;
  reset: () => void;
  adjust: (deltaMs: number) => void;
  restart: () => void;
};

const DEFAULT_DURATION_MS = 25 * 60_000;

const initialSnapshot: TimerSnapshot = {
  state: "idle",
  durationMs: DEFAULT_DURATION_MS,
  totalMs: DEFAULT_DURATION_MS,
  remainingMs: DEFAULT_DURATION_MS,
  capturedAtMs: 0,
  lastCustomMs: null,
};

/**
 * Mirrors the Rust timer. Snapshot events only arrive on state changes; while
 * running the display counts down locally from `endsAtMs` every 250 ms.
 */
export function useTimer(): TimerController {
  const [snapshot, setSnapshot] = useState<TimerSnapshot>(initialSnapshot);
  const [now, setNow] = useState(() => Date.now());
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;

    void timerService
      .subscribe((next) => {
        if (disposed) return;
        setSnapshot(next);
        setNow(Date.now());
        setReady(true);
      })
      .then((stop) => {
        if (disposed) stop();
        else unsubscribe = stop;
      })
      .catch((error) => console.error("timer subscribe failed", error));

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  const running = snapshot.state === "running";
  useEffect(() => {
    if (!running) return;
    setNow(Date.now());
    const id = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(id);
  }, [running, snapshot.endsAtMs]);

  const remainingMs =
    running && snapshot.endsAtMs !== undefined
      ? Math.max(0, Math.min(snapshot.totalMs, snapshot.endsAtMs - now))
      : snapshot.remainingMs;

  const run = useCallback((action: () => Promise<unknown>) => {
    void action().catch((error) => console.error("timer command failed", error));
  }, []);

  return {
    state: snapshot.state,
    totalMs: snapshot.totalMs,
    remainingMs,
    endsAtMs: snapshot.endsAtMs,
    lastCustomMs: snapshot.lastCustomMs,
    ready,
    setDuration: useCallback((durationMs) => run(() => timerService.set(durationMs)), [run]),
    start: useCallback(() => run(() => timerService.start()), [run]),
    pause: useCallback(() => run(() => timerService.pause()), [run]),
    resume: useCallback(() => run(() => timerService.resume()), [run]),
    reset: useCallback(() => run(() => timerService.reset()), [run]),
    adjust: useCallback((deltaMs) => run(() => timerService.adjust(deltaMs)), [run]),
    restart: useCallback(() => run(() => timerService.restart()), [run]),
  };
}
