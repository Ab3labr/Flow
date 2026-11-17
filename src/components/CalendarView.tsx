import { Plus } from "lucide-react";
import { useState } from "react";
import {
  CalendarAccount,
  CalendarEvent,
  CalendarProvider,
  calendarService,
} from "../services/mockServices";
import { CalendarConnectSheet } from "./CalendarConnect";
import { JoinButton } from "./EventCard";
import { PressButton } from "./TopBar";

export function CalendarView({
  accounts,
  events,
  onConnect,
  onReconnect,
}: {
  accounts: CalendarAccount[];
  events: CalendarEvent[];
  onConnect: (provider: CalendarProvider) => void;
  onReconnect: (id: string) => void;
}) {
  const [connectOpen, setConnectOpen] = useState(false);
  const error = accounts.find((account) => account.status === "error");

  return (
    <div className="relative h-[178px] overflow-hidden">
      {accounts.length === 0 ? (
        <div className="flex h-full flex-col items-center justify-center rounded-[14px] bg-[#121212] text-center">
          <p className="text-[13px] font-semibold text-white">Connect your calendar</p>
          <p className="mt-1 text-[11px] text-white/40">See today’s schedule and meeting links.</p>
          <PressButton
            onClick={() => setConnectOpen(true)}
            className="mt-3 h-8 rounded-full bg-white px-4 text-[11px] font-semibold text-black hover:bg-white/85"
          >
            Connect calendar
          </PressButton>
        </div>
      ) : (
        <>
          <div className="mb-2 flex h-8 items-center justify-between">
            <div>
              <p className="text-[11px] font-bold tracking-[.04em] text-[#ff453a]">WED, 4 NOV</p>
              <p className="text-[13px] font-semibold text-white">Today</p>
            </div>
            <PressButton
              onClick={() => setConnectOpen(true)}
              className="flex h-6 items-center gap-1 rounded-full bg-[#1c1c1c] px-2 text-[11px] text-white/65 hover:bg-white/15 hover:text-white"
            >
              <Plus className="h-3 w-3" /> Add
            </PressButton>
          </div>
          {error && (
            <div className="mb-1.5 flex h-8 items-center justify-between rounded-lg bg-[#121212] px-2.5">
              <span className="truncate text-[11px] text-[#ff453a]">
                Couldn't sync {error.name.split(" ")[0]}. Reconnect
              </span>
              <button
                onClick={() => onReconnect(error.id)}
                className="text-[11px] text-white/70 hover:text-white"
              >
                Reconnect
              </button>
            </div>
          )}
          {events.length === 0 ? (
            <div className="grid h-[126px] place-items-center rounded-[14px] bg-[#121212] text-[12px] text-white/40">
              No events today
            </div>
          ) : (
            <div
              className={`notch-scroll space-y-1.5 overflow-y-auto pr-1 ${error ? "h-[96px]" : "h-[138px]"}`}
            >
              {events.map((event) => {
                const provider = calendarService.getMeetingProvider(event);
                return (
                  <div
                    key={event.id}
                    className="flex h-[42px] items-center rounded-xl border border-[#1f1f1f] bg-[#121212] px-2.5"
                  >
                    <span
                      className="mr-2.5 h-6 w-[3px] rounded-full"
                      style={{ backgroundColor: event.color }}
                    />
                    <span className="w-[52px] text-[11px] tabular-nums text-white/40">
                      {event.time}
                    </span>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-[11px] font-medium text-white">{event.title}</p>
                      <p className="truncate text-[11px] text-white/40">{event.detail}</p>
                    </div>
                    {provider && <JoinButton live={event.live} provider={provider} compact />}
                  </div>
                );
              })}
            </div>
          )}
        </>
      )}
      <CalendarConnectSheet
        open={connectOpen}
        accountCount={accounts.length}
        onClose={() => setConnectOpen(false)}
        onConnected={onConnect}
      />
    </div>
  );
}
