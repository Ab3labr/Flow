import { Plus } from "lucide-react";
import { motion } from "motion/react";
import { CalendarEvent, calendarService } from "../services";
import { PressButton } from "./TopBar";

export function JoinButton({
  live = false,
  provider,
  compact = false,
}: {
  live?: boolean;
  provider?: string | null;
  compact?: boolean;
}) {
  const providerMark = provider?.startsWith("Google") ? "G" : provider === "Zoom" ? "Z" : "T";
  return (
    <motion.button
      whileTap={{ scale: 0.9, filter: "brightness(0.85)" }}
      className={`relative flex items-center gap-1 rounded-full bg-[#30d158] font-semibold text-black ${
        compact ? "h-6 px-2 text-[11px]" : "h-6 px-2 text-[11px]"
      }`}
    >
      {live && (
        <motion.span
          className="absolute inset-0 rounded-full bg-[#30d158]"
          animate={{ opacity: [0.12, 0.3, 0.12], scale: [1, 1.08, 1] }}
          transition={{ duration: 3.8, repeat: Infinity, ease: "easeInOut" }}
        />
      )}
      {provider && (
        <span className="relative grid h-3 w-3 place-items-center rounded-full bg-black/15 text-[11px] font-bold leading-none">
          {providerMark}
        </span>
      )}
      <span className="relative">Join</span>
    </motion.button>
  );
}

export function EventCard({
  event,
  eventCount = 1,
}: {
  event: CalendarEvent;
  eventCount?: number;
}) {
  const provider = calendarService.getMeetingProvider(event);
  return (
    <div className="h-full w-[148px] shrink-0 rounded-[14px] border border-[#1f1f1f] bg-[#121212] p-2">
      <div className="flex h-5 items-center justify-between">
        <p className="text-[11px] font-bold tracking-[.04em] text-[#ff453a]">WED, 4 NOV</p>
        <PressButton
          aria-label="Add event"
          className="grid h-5 w-5 place-items-center rounded-full bg-[#1c1c1c] text-white/70 hover:bg-white/15"
        >
          <Plus className="h-3 w-3" />
        </PressButton>
      </div>
      <p className="mt-0.5 text-[13px] font-semibold leading-4 text-white">
        {eventCount} {eventCount === 1 ? "event" : "events"} today
      </p>
      <div className="relative mt-1 h-[58px] rounded-[9px] bg-[#1a1a1a] py-1 pl-2.5 pr-1.5">
        <span
          className="absolute bottom-1.5 left-0 top-1.5 w-[3px] rounded-full"
          style={{ backgroundColor: event.color }}
        />
        <div className="flex min-w-0 items-baseline gap-1">
          <p className="max-w-[54px] truncate text-[11px] font-medium text-white">{event.title}</p>
          <p className="min-w-0 flex-1 truncate text-[11px] text-white/45">{event.detail}</p>
        </div>
        {provider && (
          <div className="mt-1 flex justify-end">
            <JoinButton live={event.live} provider={provider} compact />
          </div>
        )}
      </div>
    </div>
  );
}
