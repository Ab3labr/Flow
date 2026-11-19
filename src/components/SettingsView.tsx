import { Plus } from "lucide-react";
import { motion } from "motion/react";
import { useState } from "react";
import { CalendarAccount, CalendarProvider, type NotchSettings } from "../services";
import { isMockMode } from "../services/env";
import { CalendarAccountRow, CalendarConnectSheet } from "./CalendarConnect";

export type { NotchSettings };

function Toggle({ value: on, onChange }: { value: boolean; onChange: () => void }) {
  return (
    <motion.button
      type="button"
      whileTap={{ scale: 0.94 }}
      onClick={onChange}
      role="switch"
      aria-checked={on}
      className="relative box-border h-5 w-9 shrink-0 rounded-[10px] border outline-none transition-[filter] hover:brightness-110 focus-visible:ring-2 focus-visible:ring-white/70 focus-visible:ring-offset-2 focus-visible:ring-offset-[#121212]"
      style={{
        backgroundColor: on ? "#FFFFFF" : "#2A2A2A",
        borderColor: on ? "transparent" : "#444444",
      }}
    >
      <motion.span
        aria-hidden="true"
        className="absolute h-3.5 w-3.5 rounded-[7px]"
        style={{
          top: 2,
          left: 0,
          zIndex: 1,
          backgroundColor: on ? "#000000" : "#8A8A8A",
        }}
        animate={{ x: on ? 19 : 3 }}
        transition={{ type: "spring", stiffness: 500, damping: 32 }}
      />
    </motion.button>
  );
}

export function SettingsView({
  settings,
  accounts,
  onChange,
  onConnect,
  onToggleAccount,
  onDisconnect,
  onReconnect,
}: {
  settings: NotchSettings;
  accounts: CalendarAccount[];
  onChange: (settings: NotchSettings) => void;
  onConnect: (provider: CalendarProvider) => void;
  onToggleAccount: (id: string) => void;
  onDisconnect: (id: string) => void;
  onReconnect: (id: string) => void;
}) {
  const [connectOpen, setConnectOpen] = useState(false);
  const rows: { key: keyof NotchSettings; label: string; demo?: boolean }[] = [
    { key: "topEdge", label: "Open at top edge" },
    { key: "altAlt", label: "Open with Alt Alt" },
    { key: "hideInFullscreen", label: "Hide in fullscreen apps" },
    { key: "launchAtStartup", label: "Launch at startup" },
    { key: "upcomingEvent", label: "Demo: upcoming event", demo: true },
    { key: "noCalendars", label: "Demo: no calendars connected", demo: true },
    { key: "compactBar", label: "Compact music bar" },
  ];

  return (
    <div className="relative h-[288px] overflow-hidden">
      <div className="notch-scroll h-full overflow-y-auto pr-1">
        <p className="mb-1.5 text-[11px] font-semibold uppercase tracking-[.08em] text-white/40">
          Notch settings
        </p>
        <div className="overflow-hidden rounded-[14px] border border-[#1f1f1f] bg-[#121212]">
          {rows
            .filter((row) => !row.demo || isMockMode)
            .map(({ key, label }) => (
              <div
                key={key}
                className="flex h-8 items-center justify-between border-b border-white/[0.06] px-2.5 last:border-0"
              >
                <span
                  className={`text-[11px] transition-colors ${
                    settings[key] ? "text-white" : "text-white/50"
                  }`}
                >
                  {label}
                </span>
                <div className="flex items-center gap-2">
                  {key === "altAlt" && (
                    <input
                      aria-label="Open shortcut"
                      value={settings.shortcut}
                      readOnly
                      onKeyDown={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        const pressedKey =
                          event.key === " "
                            ? "Space"
                            : event.key.length === 1
                              ? event.key.toUpperCase()
                              : event.key.replace("Control", "Ctrl");
                        const parts = [
                          event.ctrlKey && pressedKey !== "Ctrl" ? "Ctrl" : "",
                          event.altKey && pressedKey !== "Alt" ? "Alt" : "",
                          event.shiftKey && pressedKey !== "Shift" ? "Shift" : "",
                          event.metaKey && pressedKey !== "Meta" ? "Meta" : "",
                          pressedKey,
                        ].filter(Boolean);
                        onChange({ ...settings, shortcut: parts.join(" + ") });
                      }}
                      className="w-[58px] rounded-md border border-white/10 bg-[#1c1c1c] px-1 py-0.5 text-center text-[11px] text-white/60"
                    />
                  )}
                  <Toggle
                    value={settings[key] as boolean}
                    onChange={() => onChange({ ...settings, [key]: !settings[key] })}
                  />
                </div>
              </div>
            ))}
        </div>

        <div className="mb-1.5 mt-3 flex items-center justify-between">
          <p className="text-[11px] font-semibold uppercase tracking-[.08em] text-white/40">
            Calendars
          </p>
          <button
            onClick={() => setConnectOpen(true)}
            className="flex items-center gap-1 text-[11px] text-white/55 hover:text-white"
          >
            <Plus className="h-3 w-3" /> Add
          </button>
        </div>
        <div className="space-y-1.5">
          {accounts.length ? (
            accounts.map((account) => (
              <CalendarAccountRow
                key={account.id}
                account={account}
                onToggle={() => onToggleAccount(account.id)}
                onDisconnect={() => onDisconnect(account.id)}
                onReconnect={() => onReconnect(account.id)}
              />
            ))
          ) : (
            <button
              onClick={() => setConnectOpen(true)}
              className="h-12 w-full rounded-xl bg-[#121212] text-[11px] text-white/50 hover:text-white"
            >
              Connect calendar
            </button>
          )}
        </div>

        <div className="mt-2 flex h-9 items-center justify-between rounded-xl bg-[#121212] px-2.5">
          <div className="min-w-0">
            <p className="text-[11px] text-white/80">Notes folder</p>
            <p className="truncate text-[11px] text-white/40">Documents\Notch Notes</p>
          </div>
          <button className="rounded-full bg-[#1c1c1c] px-2.5 py-1 text-[11px] text-white/65 hover:bg-white/15">
            Open folder
          </button>
        </div>
      </div>
      <CalendarConnectSheet
        open={connectOpen}
        accountCount={accounts.length}
        onClose={() => setConnectOpen(false)}
        onConnected={onConnect}
      />
    </div>
  );
}
