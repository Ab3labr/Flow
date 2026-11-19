import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { CalendarDays, Clock, Mic, Music, Timer } from "lucide-react";
import { useCallback, useLayoutEffect, useRef } from "react";
import { Equalizer } from "../components/CollapsedPill";
import { useDominantColor } from "./useDominantColor";
import type { LiveActivity, LiveLeading, LiveTrailing } from "./types";

export const COMPACT_BAR_HEIGHT = 30;
export const COMPACT_BAR_MIN_WIDTH = 150;
export const COMPACT_BAR_MAX_WIDTH = 260;
/** Bar + window margin used by Rust `set_ui_mode("pill")`. */
export const COMPACT_BAR_WINDOW_MARGIN = { x: 32, y: 18 } as const;

const FALLBACK_ACCENT = "#f09ab7";

const LEADING_ICONS: Record<string, typeof Music> = {
  music: Music,
  recording: Mic,
  timer: Timer,
  meeting: CalendarDays,
};

function Leading({ leading }: { leading: LiveLeading }) {
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
  const Icon = LEADING_ICONS[leading.name.toLowerCase()] ?? Music;
  return (
    <span className="grid h-5 w-5 shrink-0 place-items-center rounded-[6px] bg-white/10">
      <Icon className="h-3 w-3 text-white/80" strokeWidth={1.8} />
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
  const reducedMotion = useReducedMotion();
  const barRef = useRef<HTMLDivElement>(null);
  const playing = activity.playing ?? false;
  const artColor = activity.leading.type === "image" ? activity.leading.url : null;
  const dominant = useDominantColor(artColor);
  const color = dominant ?? activity.accent ?? FALLBACK_ACCENT;

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
      className="relative flex h-[30px] w-max min-w-[150px] max-w-[260px] cursor-pointer items-center gap-1.5 rounded-b-[15px] bg-black pl-1.5 pr-2.5 outline-none focus-visible:ring-2 focus-visible:ring-white/70"
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

      <Leading leading={activity.leading} />

      <div className="relative min-w-0 flex-1 overflow-hidden">
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.span
            key={`${activity.id}-${activity.label}`}
            className="block truncate text-[11px] font-medium leading-none text-white"
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
    </div>
  );
}
