import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { Battery, CalendarDays, Camera, Clock, Mic, Music, Pause, Timer, Zap } from "lucide-react";
import { useCallback, useLayoutEffect, useRef } from "react";
import { Equalizer } from "./Equalizer";
import {
  COMPACT_BAR_HEIGHT,
  COMPACT_BAR_MAX_WIDTH,
  COMPACT_BAR_MIN_WIDTH,
  COMPACT_BAR_WINDOW_MARGIN,
} from "./geometry";
import { useDominantColor } from "./useDominantColor";
import type { LiveActivity, LiveLeading, LiveTrailing } from "./types";

export {
  COMPACT_BAR_HEIGHT,
  COMPACT_BAR_MAX_WIDTH,
  COMPACT_BAR_MIN_WIDTH,
  COMPACT_BAR_WINDOW_MARGIN,
} from "./geometry";

const FALLBACK_ACCENT = "#f09ab7";
const RING_CIRCUMFERENCE = 2 * Math.PI * 6;

const LEADING_ICONS: Record<string, typeof Music> = {
  music: Music,
  recording: Mic,
  mic: Mic,
  camera: Camera,
  timer: Timer,
  "timer-done": Timer,
  meeting: CalendarDays,
  charging: Zap,
  battery: Battery,
};

function Leading({
  leading,
  playing,
  color,
  dimmed,
  reducedMotion,
}: {
  leading: LiveLeading;
  playing: boolean;
  color: string;
  dimmed: boolean;
  reducedMotion: boolean;
}) {
  if (leading.type === "image") {
    return (
      <img
        src={leading.url}
        alt=""
        draggable={false}
        className="h-5 w-5 shrink-0 rounded-[6px] object-cover"
      />
    );
  }
  if (leading.type === "initial") {
    const text = leading.text.trim().charAt(0).toUpperCase() || "?";
    return (
      <span
        className="grid h-5 w-5 shrink-0 place-items-center rounded-[6px] text-[9px] font-semibold text-white/90"
        style={{
          background: `linear-gradient(135deg, ${leading.colors.join(", ") || "#3a3a3a, #141414"})`,
        }}
      >
        {text}
      </span>
    );
  }
  if (leading.type === "recordingDot") {
    return (
      <span className={`relative grid h-5 w-5 shrink-0 place-items-center ${dimmed ? "opacity-60" : ""}`}>
        <motion.span
          className="absolute h-2.5 w-2.5 rounded-full"
          style={{ backgroundColor: leading.color }}
          animate={{ opacity: [0.55, 0, 0.55], scale: [1, 2, 1] }}
          transition={{
            duration: reducedMotion ? 0 : 1.8,
            repeat: reducedMotion ? 0 : Infinity,
          }}
        />
        <span
          className="relative h-2.5 w-2.5 rounded-full"
          style={{ backgroundColor: leading.color }}
        />
      </span>
    );
  }
  if (leading.type === "icons") {
    return (
      <span className="flex h-5 w-5 shrink-0 items-center justify-center gap-0.5 rounded-[6px] bg-white/10">
        {leading.icons.map((icon, index) => {
          const Icon = LEADING_ICONS[icon.name.toLowerCase()] ?? Music;
          return (
            <Icon
              key={`${icon.name}-${index}`}
              className="h-2.5 w-2.5"
              style={{ color: icon.color }}
              strokeWidth={1.8}
            />
          );
        })}
      </span>
    );
  }
  if (leading.type === "ring") {
    const progress = Math.max(0, Math.min(1, leading.progress));
    return (
      <span className={`relative grid h-5 w-5 shrink-0 place-items-center ${dimmed ? "opacity-60" : ""}`}>
        <svg className="absolute h-[14px] w-[14px] -rotate-90" viewBox="0 0 16 16">
          <circle cx="8" cy="8" r="6" fill="none" stroke="rgba(255,255,255,.16)" strokeWidth="2" />
          <motion.circle
            cx="8"
            cy="8"
            r="6"
            fill="none"
            stroke={color}
            strokeWidth="2"
            strokeLinecap="round"
            strokeDasharray={RING_CIRCUMFERENCE}
            animate={{ strokeDashoffset: RING_CIRCUMFERENCE * (1 - progress) }}
            transition={{ duration: reducedMotion ? 0.15 : 0.3, ease: "easeOut" }}
          />
        </svg>
        {!playing && (
          <Pause className="relative h-2.5 w-2.5 fill-white text-white" strokeWidth={2} />
        )}
      </span>
    );
  }
  if (leading.type === "pulse") {
    const Icon = LEADING_ICONS[leading.name.toLowerCase()] ?? Timer;
    return (
      <motion.span
        className="grid h-5 w-5 shrink-0 place-items-center rounded-[6px] bg-white/10"
        animate={{ scale: [1, 1.35, 1], opacity: [0.5, 1, 0.5] }}
        transition={{
          duration: reducedMotion ? 0 : 1.5,
          repeat: reducedMotion ? 0 : Infinity,
        }}
      >
        <Icon
          className="h-3 w-3"
          style={{ color: leading.color ?? color }}
          strokeWidth={1.8}
        />
      </motion.span>
    );
  }
  const Icon = LEADING_ICONS[leading.name.toLowerCase()] ?? Music;
  return (
    <span className="grid h-5 w-5 shrink-0 place-items-center rounded-[6px] bg-white/10">
      <Icon
        className="h-3 w-3 text-white/80"
        style={{ color: leading.color }}
        strokeWidth={1.8}
      />
    </span>
  );
}

function Trailing({
  trailing,
  playing,
  color,
}: {
  trailing: LiveTrailing;
  playing: boolean;
  color: string;
}) {
  if (typeof trailing === "object") {
    return (
      <span
        className="min-w-0 truncate text-[11px] font-medium tabular-nums"
        style={{ color: trailing.color }}
      >
        {trailing.value}
      </span>
    );
  }
  if (trailing === "equalizer") return <Equalizer playing={playing} color={color} />;
  if (trailing === "dot") {
    return (
      <span
        className="mr-0.5 h-1.5 w-1.5 shrink-0 rounded-full"
        style={{ backgroundColor: color }}
      />
    );
  }
  if (trailing === "time") {
    return <Clock className="h-3 w-3 shrink-0" style={{ color }} strokeWidth={1.8} />;
  }
  if (trailing === "pause") {
    return <Pause className="h-3 w-3 shrink-0 fill-white/55 text-white/55" strokeWidth={2} />;
  }
  return null;
}

export function CompactBar({
  activity,
  onWidthChange,
  onClick,
}: {
  activity: LiveActivity;
  onWidthChange: (width: number) => void;
  onClick: () => void;
}) {
  const reducedMotion = Boolean(useReducedMotion());
  const barRef = useRef<HTMLDivElement>(null);
  const playing = activity.playing ?? false;
  const artColor = activity.leading.type === "image" ? activity.leading.url : null;
  const dominant = useDominantColor(artColor);
  const color = dominant ?? activity.accent ?? FALLBACK_ACCENT;
  const maxWidth = activity.maxWidth ?? COMPACT_BAR_MAX_WIDTH;
  const dimmed = activity.kind === "timer" && !playing;
  const customContent = activity.render?.({ activity, playing, color, maxWidth });

  const reportWidth = useCallback(() => {
    // offsetWidth is the layout width; getBoundingClientRect is affected by
    // the prototype's desktop scale transform.
    const width = barRef.current?.offsetWidth ?? 0;
    if (width > 0) onWidthChange(width);
  }, [onWidthChange]);

  useLayoutEffect(() => {
    reportWidth();
    const element = barRef.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(reportWidth);
    observer.observe(element);
    return () => observer.disconnect();
  }, [reportWidth]);

  const revealActionsOnHover = activity.hover === "actions";

  return (
    <div
      ref={barRef}
      role="button"
      tabIndex={0}
      aria-label={`${activity.label} — open Float`}
      onClick={onClick}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onClick();
        }
      }}
      className="group relative flex h-[30px] w-max shrink-0 cursor-pointer items-center gap-1.5 rounded-b-[15px] bg-black pl-1.5 pr-2.5 outline-none focus-visible:ring-2 focus-visible:ring-white/70"
      style={{ minWidth: COMPACT_BAR_MIN_WIDTH, maxWidth }}
    >
      <motion.div
        aria-hidden="true"
        className="pointer-events-none absolute inset-x-4 -bottom-1 h-2.5 rounded-full blur-[8px]"
        style={{ backgroundColor: color }}
        animate={
          playing && !reducedMotion
            ? { opacity: [0.16, 0.28, 0.16], scaleX: [0.94, 1, 0.94] }
            : { opacity: 0.28, scaleX: 1 }
        }
        transition={
          playing && !reducedMotion
            ? { duration: 3, repeat: Infinity, ease: "easeInOut" }
            : { duration: 0.2 }
        }
      />

      {customContent ?? (
        <>
          <Leading
            leading={activity.leading}
            playing={playing}
            color={color}
            dimmed={dimmed}
            reducedMotion={reducedMotion}
          />

          <div className="relative min-w-0 flex-1 overflow-hidden">
            <AnimatePresence mode="popLayout" initial={false}>
              <motion.span
                key={`${activity.id}-${activity.label}`}
                className={`block truncate text-[11px] font-medium leading-none ${
                  activity.kind === "timer" || activity.kind === "recording" ? "tabular-nums" : ""
                } ${dimmed ? "text-white/50" : "text-white"}`}
                initial={{ opacity: 0, x: 5 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -5 }}
                transition={{ duration: reducedMotion ? 0.15 : 0.26, ease: "easeOut" }}
              >
                {activity.label}
              </motion.span>
            </AnimatePresence>
          </div>

          {activity.trailing && activity.trailing !== "none" && (
            <Trailing trailing={activity.trailing} playing={playing} color={color} />
          )}

          {activity.actions?.length ? (
            <div
              className={`flex shrink-0 items-center gap-1 ${
                revealActionsOnHover
                  ? "pointer-events-none opacity-0 transition-opacity duration-150 group-hover:pointer-events-auto group-hover:opacity-100 group-focus-within:pointer-events-auto group-focus-within:opacity-100"
                  : ""
              }`}
            >
              {activity.actions.map((action) => (
                <button
                  key={action.id}
                  type="button"
                  aria-label={action.label}
                  onClick={(event) => {
                    event.stopPropagation();
                    action.onClick();
                  }}
                  onKeyDown={(event) => event.stopPropagation()}
                  className="grid shrink-0 place-items-center rounded-full outline-none focus-visible:ring-2 focus-visible:ring-white/70"
                >
                  {action.icon}
                </button>
              ))}
            </div>
          ) : null}
        </>
      )}
    </div>
  );
}
