import { Minus, Plus, RotateCcw } from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import type { TimerController } from "../live/useTimer";
import { timerService } from "../services";
import { PressButton } from "./TopBar";

const MINUTE_MS = 60_000;
const MIN_DURATION_MS = 1_000;
const MAX_DURATION_MS = 86_399_000;

function clampDuration(durationMs: number): number {
  return Math.min(MAX_DURATION_MS, Math.max(MIN_DURATION_MS, Math.round(durationMs)));
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

/** `M:SS`, or `H:MM:SS` from one hour (matches the Rust toast format). */
function formatTimer(durationMs: number): string {
  const total = Math.max(0, Math.ceil(durationMs / 1_000));
  const hours = Math.floor(total / 3_600);
  const minutes = Math.floor((total % 3_600) / 60);
  const seconds = total % 60;
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(seconds)}` : `${minutes}:${pad(seconds)}`;
}

/** Chip text: whole minutes as "7m", otherwise "1:30". */
function chipLabel(durationMs: number): string {
  const total = Math.round(durationMs / 1_000);
  if (total % 60 === 0) return `${total / 60}m`;
  return `${Math.floor(total / 60)}:${pad(total % 60)}`;
}

/** Microwave entry: the last two digits are seconds ("1500" -> 15:00). */
function digitsToDuration(digits: string): number {
  const padded = digits.padStart(3, "0");
  const seconds = Number(padded.slice(-2));
  const minutes = Number(padded.slice(0, -2) || "0");
  return clampDuration((minutes * 60 + seconds) * 1_000);
}

function HoldButton({
  direction,
  onChange,
  className = "",
}: {
  direction: -1 | 1;
  onChange: (minutes: number) => void;
  className?: string;
}) {
  const timer = useRef<number | undefined>(undefined);
  const repeats = useRef(0);

  const stop = useCallback(() => {
    window.clearTimeout(timer.current);
    timer.current = undefined;
  }, []);

  const start = (event: ReactPointerEvent<HTMLButtonElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    const step = event.shiftKey ? 5 : 1;
    onChange(direction * step);
    repeats.current = 0;
    const repeat = () => {
      repeats.current += 1;
      onChange(direction * step);
      timer.current = window.setTimeout(repeat, Math.max(60, 190 - repeats.current * 14));
    };
    timer.current = window.setTimeout(repeat, 420);
  };

  useEffect(() => stop, [stop]);

  return (
    <motion.button
      type="button"
      whileTap={{ scale: 0.9 }}
      onPointerDown={start}
      onPointerUp={stop}
      onPointerCancel={stop}
      onPointerLeave={stop}
      className={`absolute top-1/2 grid h-7 w-7 -translate-y-1/2 place-items-center rounded-full bg-white/[0.07] text-white/60 transition-opacity duration-[120ms] hover:bg-white/15 hover:text-white focus-visible:opacity-100 group-hover:opacity-100 group-focus-within:opacity-100 ${className}`}
      aria-label={direction < 0 ? "Subtract one minute" : "Add one minute"}
    >
      {direction < 0 ? (
        <Minus className="h-3.5 w-3.5" strokeWidth={2} />
      ) : (
        <Plus className="h-3.5 w-3.5" strokeWidth={2} />
      )}
    </motion.button>
  );
}

export function TimerView({
  timer,
  onStarted,
}: {
  timer: TimerController;
  /** Called after a start/restart so the panel can collapse into the bar. */
  onStarted: () => void;
}) {
  const reducedMotion = useReducedMotion();
  const [editing, setEditing] = useState(false);
  const [digits, setDigits] = useState("");
  const editorRef = useRef<HTMLDivElement>(null);

  const { state } = timer;
  const idle = state === "idle";
  const running = state === "running";
  const paused = state === "paused";
  const done = state === "done";
  const adjustable = !done;

  const presets = timerService.getPresets();
  const chips = presets.map((minutes) => ({ durationMs: minutes * MINUTE_MS, label: `${minutes}m` }));
  const lastCustom = timer.lastCustomMs;
  if (lastCustom != null && !presets.includes(lastCustom / MINUTE_MS)) {
    chips.push({ durationMs: lastCustom, label: chipLabel(lastCustom) });
  }

  const displayMs = editing && digits ? digitsToDuration(digits) : timer.remainingMs;
  const primaryLabel = running ? "Pause" : paused ? "Resume" : done ? "Restart" : "Start";
  const progress = done ? 0 : timer.totalMs > 0 ? timer.remainingMs / timer.totalMs : 0;
  const endsLabel =
    running && timer.endsAtMs !== undefined
      ? `Ends ${new Date(timer.endsAtMs).toLocaleTimeString([], {
          hour: "numeric",
          minute: "2-digit",
        })}`
      : paused
        ? "Paused"
        : done
          ? "Time's up"
          : "";

  const adjustMinutes = useCallback(
    (minutes: number) => timer.adjust(minutes * MINUTE_MS),
    [timer.adjust],
  );

  const commitEditing = useCallback(() => {
    if (digits) timer.setDuration(digitsToDuration(digits));
    setEditing(false);
    setDigits("");
  }, [digits, timer.setDuration]);

  const cancelEditing = useCallback(() => {
    setEditing(false);
    setDigits("");
  }, []);

  // Digit entry: type to fill from the right, Enter saves, Esc/click-away cancels.
  useEffect(() => {
    if (!editing) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (/^[0-9]$/.test(event.key)) {
        event.preventDefault();
        setDigits((current) =>
          (current + event.key).replace(/^0+(?=\d)/, "").slice(-6),
        );
        return;
      }
      if (event.key === "Backspace") {
        event.preventDefault();
        setDigits((current) => current.slice(0, -1));
        return;
      }
      if (event.key === "Enter") {
        event.preventDefault();
        commitEditing();
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        cancelEditing();
        return;
      }
      if (event.key === " ") event.preventDefault();
    };

    const onPointerDown = (event: PointerEvent) => {
      if (editorRef.current && !editorRef.current.contains(event.target as Node)) {
        cancelEditing();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("pointerdown", onPointerDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("pointerdown", onPointerDown);
    };
  }, [editing, commitEditing, cancelEditing]);

  // A timer started elsewhere (Space, compact bar) leaves edit mode.
  useEffect(() => {
    if (!idle && editing) cancelEditing();
  }, [idle, editing, cancelEditing]);

  // Space starts, pauses or resumes while the Timer tab is mounted.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== " " || event.repeat || editing) return;
      const target = event.target as HTMLElement | null;
      if (target?.closest("input, textarea, [contenteditable='true']")) return;
      event.preventDefault();
      if (state === "running") {
        timer.pause();
      } else if (state === "paused") {
        timer.resume();
      } else if (state === "idle") {
        timer.start();
        onStarted();
      } else if (state === "done") {
        timer.restart();
        onStarted();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [editing, state, timer.pause, timer.resume, timer.start, timer.restart, onStarted]);

  const primaryAction = () => {
    if (state === "running") {
      timer.pause();
    } else if (state === "paused") {
      timer.resume();
    } else if (state === "done") {
      timer.restart();
      onStarted();
    } else {
      timer.start();
      onStarted();
    }
  };

  return (
    <div className="flex h-[148px] flex-col items-center justify-center gap-2.5">
      <div className="group relative flex h-12 items-center justify-center">
        {adjustable && (
          <HoldButton direction={-1} onChange={adjustMinutes} className="right-full mr-5 opacity-0" />
        )}
        <div ref={editorRef} className="relative flex items-center">
          <motion.button
            type="button"
            aria-label={idle ? "Edit timer duration" : formatTimer(displayMs)}
            onClick={() => {
              if (idle && !editing) {
                setDigits("");
                setEditing(true);
              }
            }}
            onKeyDown={(event) => {
              if (editing) return;
              if (event.key === "ArrowUp" || event.key === "ArrowDown") {
                event.preventDefault();
                adjustMinutes(event.key === "ArrowUp" ? 1 : -1);
              }
            }}
            onWheel={(event) => {
              if (editing) return;
              adjustMinutes(event.deltaY < 0 ? 1 : -1);
            }}
            className={`flex items-center justify-center rounded-lg text-[44px] font-semibold leading-none tracking-[-.04em] tabular-nums text-white ${
              idle ? "cursor-text" : "cursor-default"
            }`}
          >
            <motion.span
              animate={
                done && !reducedMotion ? { opacity: [1, 0.55, 1] } : { opacity: paused ? 0.6 : 1 }
              }
              transition={
                done && !reducedMotion
                  ? { duration: 1.6, repeat: Infinity, ease: "easeInOut" }
                  : { duration: 0.16 }
              }
            >
              {formatTimer(displayMs)}
            </motion.span>
            {editing && (
              <span className="ml-1 inline-block h-[34px] w-[2px] shrink-0 bg-white" />
            )}
          </motion.button>
        </div>
        {adjustable && (
          <HoldButton direction={1} onChange={adjustMinutes} className="left-full ml-5 opacity-0" />
        )}
      </div>

      <div className="relative h-10 w-[220px]">
        <motion.div
          aria-hidden={!idle}
          inert={!idle}
          animate={{ opacity: idle ? 1 : 0 }}
          transition={{ duration: reducedMotion ? 0 : 0.16 }}
          className={`absolute inset-0 flex items-center justify-center gap-1.5 ${
            idle ? "" : "pointer-events-none"
          }`}
        >
          {chips.map((chip) => {
            const selected = idle && timer.totalMs === chip.durationMs;
            return (
              <PressButton
                key={chip.durationMs}
                type="button"
                onClick={() => timer.setDuration(chip.durationMs)}
                aria-pressed={selected}
                className={`h-7 rounded-full px-2.5 text-[11px] transition-colors ${
                  selected ? "bg-[#1c1c1c] text-white" : "text-white/45 hover:text-white/80"
                }`}
              >
                {chip.label}
              </PressButton>
            );
          })}
        </motion.div>

        <motion.div
          aria-hidden={idle}
          animate={{ opacity: idle ? 0 : 1 }}
          transition={{ duration: reducedMotion ? 0 : 0.16 }}
          className={`absolute inset-0 flex flex-col items-center justify-center ${
            idle ? "pointer-events-none" : ""
          }`}
        >
          <div className="h-[2px] w-[220px] overflow-hidden rounded-full bg-white/15">
            <div
              className="h-full rounded-full bg-white transition-[width] duration-200 ease-linear"
              style={{ width: `${Math.max(0, Math.min(1, progress)) * 100}%` }}
            />
          </div>
          <p className="mt-1.5 h-[13px] text-[11px] leading-[13px] text-white/40">{endsLabel}</p>
        </motion.div>
      </div>

      <div className="flex h-9 items-center justify-center gap-2.5">
        <AnimatePresence initial={false}>
          {!idle && (
            <motion.button
              key="reset"
              type="button"
              onClick={() => timer.reset()}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: reducedMotion ? 0 : 0.12 }}
              whileTap={{ scale: 0.92 }}
              aria-label="Reset timer"
              className="grid h-9 w-9 place-items-center rounded-full bg-[#1c1c1c] text-white/70 hover:bg-white/15 hover:text-white"
            >
              <RotateCcw className="h-4 w-4" strokeWidth={1.8} />
            </motion.button>
          )}
        </AnimatePresence>
        <PressButton
          type="button"
          onClick={primaryAction}
          className="relative h-9 w-[120px] overflow-hidden rounded-full bg-white text-[12px] font-semibold text-black hover:bg-white/85"
        >
          <AnimatePresence initial={false}>
            <motion.span
              key={primaryLabel}
              className="absolute inset-0 grid place-items-center"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: reducedMotion ? 0 : 0.16 }}
            >
              {primaryLabel}
            </motion.span>
          </AnimatePresence>
        </PressButton>
      </div>
    </div>
  );
}
