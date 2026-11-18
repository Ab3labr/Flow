import { Minus, Plus } from "lucide-react";
import { motion } from "motion/react";
import { PointerEvent, useEffect, useRef, useState } from "react";
import { timerService } from "../services";
import { PressButton } from "./TopBar";

function HoldButton({
  direction,
  onChange,
}: {
  direction: -1 | 1;
  onChange: (minutes: number) => void;
}) {
  const timer = useRef<number | undefined>(undefined);
  const repeats = useRef(0);

  const stop = () => window.clearTimeout(timer.current);
  const start = (event: PointerEvent<HTMLButtonElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    onChange(direction);
    repeats.current = 0;
    const repeat = () => {
      repeats.current += 1;
      onChange(direction * 5);
      timer.current = window.setTimeout(repeat, Math.max(60, 190 - repeats.current * 14));
    };
    timer.current = window.setTimeout(repeat, 420);
  };

  useEffect(() => stop, []);

  return (
    <motion.button
      whileTap={{ scale: 0.9 }}
      onPointerDown={start}
      onPointerUp={stop}
      onPointerCancel={stop}
      onPointerLeave={stop}
      className="grid h-7 w-7 place-items-center rounded-full bg-[#1c1c1c] text-white/60 hover:bg-white/15 hover:text-white"
      aria-label={direction < 0 ? "Subtract time" : "Add time"}
    >
      {direction < 0 ? <Minus className="h-3.5 w-3.5" /> : <Plus className="h-3.5 w-3.5" />}
    </motion.button>
  );
}

export function TimerView({
  total,
  remaining,
  running,
  complete,
  onPreset,
  onToggle,
  onAddTime,
}: {
  total: number;
  remaining: number;
  running: boolean;
  complete: boolean;
  onPreset: (minutes: number) => void;
  onToggle: () => void;
  onAddTime: (seconds: number) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [digits, setDigits] = useState("");
  const [customTimes, setCustomTimes] = useState<number[]>(timerService.getCustomTimes);
  const [handleTick, setHandleTick] = useState(0);
  const ringRef = useRef<HTMLDivElement>(null);
  const minutes = Math.max(1, Math.round(remaining / 60));
  const progress = running ? (total > 0 ? remaining / total : 0) : minutes / 120;
  const display = `${String(Math.floor(remaining / 60)).padStart(2, "0")}:${String(remaining % 60).padStart(2, "0")}`;
  const angle = progress * 360 - 90;
  const handleX = `${50 + Math.cos((angle * Math.PI) / 180) * 43.75}%`;
  const handleY = `${50 + Math.sin((angle * Math.PI) / 180) * 43.75}%`;

  const rememberCustom = (value: number) => {
    setCustomTimes(timerService.rememberCustomTime(value));
  };

  const setMinutes = (value: number, remember = false) => {
    const clamped = Math.max(1, Math.min(120, Math.round(value)));
    onPreset(clamped);
    if (remember) rememberCustom(clamped);
  };

  const adjust = (amount: number) => setMinutes(minutes + amount, true);

  const dragRing = (event: PointerEvent<HTMLDivElement>) => {
    if (running || !ringRef.current) return;
    const rect = ringRef.current.getBoundingClientRect();
    const x = event.clientX - (rect.left + rect.width / 2);
    const y = event.clientY - (rect.top + rect.height / 2);
    let degrees = (Math.atan2(y, x) * 180) / Math.PI + 90;
    if (degrees <= 0) degrees += 360;
    const next = Math.max(1, Math.min(120, Math.round(degrees / 3)));
    if (next % 5 === 0 && next !== handleTick) setHandleTick(next);
    setMinutes(next);
  };

  const commitDigits = () => {
    setEditing(false);
    if (!digits) return;
    const padded = digits.padStart(3, "0");
    const seconds = Number(padded.slice(-2));
    const typedMinutes = Number(padded.slice(0, -2));
    const totalSeconds = Math.min(7200, Math.max(60, typedMinutes * 60 + seconds));
    const roundedMinutes = Math.max(1, Math.round(totalSeconds / 60));
    setMinutes(roundedMinutes, true);
    setDigits("");
  };

  return (
    <div className="flex h-[198px] items-center justify-center gap-6">
      <motion.div
        ref={ringRef}
        className={`relative grid h-32 w-32 place-items-center ${running ? "" : "cursor-crosshair"}`}
        onPointerDown={(event) => {
          if (!running) {
            event.currentTarget.setPointerCapture(event.pointerId);
            dragRing(event);
          }
        }}
        onPointerMove={(event) =>
          event.currentTarget.hasPointerCapture(event.pointerId) && dragRing(event)
        }
        animate={
          complete
            ? { scale: [1, 1.05, 1], filter: ["brightness(1)", "brightness(1.5)", "brightness(1)"] }
            : {}
        }
      >
        <svg className="absolute inset-0 -rotate-90" viewBox="0 0 144 144">
          <circle cx="72" cy="72" r="63" fill="none" stroke="#232323" strokeWidth="6" />
          <motion.circle
            cx="72"
            cy="72"
            r="63"
            fill="none"
            stroke="white"
            strokeLinecap="round"
            strokeWidth="6"
            pathLength="1"
            animate={{ pathLength: progress }}
            transition={{ duration: running ? 1 : 0.12, ease: running ? "linear" : "easeOut" }}
          />
        </svg>
        {!running && (
          <motion.span
            className="absolute h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-black bg-white shadow-sm"
            style={{ left: handleX, top: handleY }}
            animate={{ scale: handleTick % 5 === 0 ? [1, 1.22, 1] : 1 }}
            transition={{ duration: 0.12 }}
          />
        )}
        <div className="relative flex items-center gap-1.5">
          {!running && <HoldButton direction={-1} onChange={adjust} />}
          {editing ? (
            <input
              autoFocus
              inputMode="numeric"
              value={digits}
              onChange={(event) => setDigits(event.target.value.replace(/\D/g, "").slice(0, 5))}
              onBlur={commitDigits}
              onKeyDown={(event) => event.key === "Enter" && commitDigits()}
              className="h-8 w-16 bg-transparent text-center text-[25px] font-semibold tracking-[-.04em] tabular-nums text-white"
              placeholder="00:00"
            />
          ) : (
            <button
              onClick={() => {
                if (!running) {
                  setDigits("");
                  setEditing(true);
                }
              }}
              className="h-8 w-16 overflow-hidden text-[25px] font-semibold leading-8 tracking-[-.04em] tabular-nums text-white"
            >
              <motion.span
                key={display}
                className="block"
                initial={{ y: 22, opacity: 0 }}
                animate={{ y: 0, opacity: 1 }}
                transition={{ duration: 0.28 }}
              >
                {display}
              </motion.span>
            </button>
          )}
          {!running && <HoldButton direction={1} onChange={adjust} />}
        </div>
      </motion.div>
      <div className="flex w-[196px] flex-col">
        <p className="text-[11px] font-semibold uppercase tracking-[.08em] text-white/40">
          Focus timer
        </p>
        <div className="mt-2 flex flex-wrap gap-1.5">
          {timerService.getPresets().map((value) => (
            <PressButton
              key={value}
              onClick={() => setMinutes(value)}
              className="h-7 rounded-full bg-[#1c1c1c] px-3 text-[11px] text-white/65 hover:bg-white/15 hover:text-white"
            >
              {value} min
            </PressButton>
          ))}
          <PressButton
            onClick={() => {
              setDigits("");
              setEditing(true);
            }}
            className="h-7 rounded-full bg-[#1c1c1c] px-3 text-[11px] text-white/65 hover:bg-white/15 hover:text-white"
          >
            Custom
          </PressButton>
          {customTimes.map((value) => (
            <PressButton
              key={`custom-${value}`}
              onClick={() => setMinutes(value)}
              className="h-7 rounded-full bg-[#1c1c1c] px-3 text-[11px] text-white/65 hover:bg-white/15 hover:text-white"
            >
              {value} min
            </PressButton>
          ))}
          {running && (
            <PressButton
              onClick={() => onAddTime(60)}
              className="h-7 rounded-full bg-[#1c1c1c] px-3 text-[11px] text-white/65 hover:bg-white/15 hover:text-white"
            >
              +1 min
            </PressButton>
          )}
        </div>
        <PressButton
          onClick={onToggle}
          className="mt-3 h-9 rounded-full bg-white text-[12px] font-semibold text-black hover:bg-white/85"
        >
          {running ? "Pause" : remaining === 0 ? "Restart" : "Start"}
        </PressButton>
      </div>
    </div>
  );
}
