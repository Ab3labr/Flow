import { Plus } from "lucide-react";
import { motion } from "motion/react";
import { useState, type KeyboardEvent } from "react";
import { CalendarAccount, CalendarProvider, type NotchSettings } from "../services";
import { isMockMode } from "../services/env";
import {
  formatPressedKey,
  shortcutConflictsWithHotkeys,
  validateHotkeys,
  type ShortcutField,
} from "../services/shortcuts";
import type { HotkeyFailures } from "../services/uiService";
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

function ShortcutInput({
  label,
  value,
  error,
  onChange,
}: {
  label: string;
  value: string;
  error?: string | null;
  onChange: (value: string) => void;
}) {
  const capture = (event: KeyboardEvent<HTMLInputElement>) => {
    event.preventDefault();
    event.stopPropagation();
    onChange(formatPressedKey(event.key, event));
  };
  return (
    <input
      aria-label={label}
      title={error ?? undefined}
      value={value}
      readOnly
      onKeyDown={capture}
      className={`w-[72px] shrink-0 rounded-md border bg-[#1c1c1c] px-1 py-0.5 text-center text-[11px] outline-none focus-visible:ring-2 focus-visible:ring-white/70 ${
        error ? "border-[#ff453a] text-[#ff453a]" : "border-white/10 text-white/60"
      }`}
    />
  );
}

const HOTKEY_ROWS: { field: ShortcutField; label: string }[] = [
  { field: "hotkeyNotes", label: "Open Notes" },
  { field: "hotkeyNext", label: "Next track" },
  { field: "hotkeyPrev", label: "Previous track" },
];

const FAILURE_KEY: Record<ShortcutField, keyof HotkeyFailures> = {
  hotkeyNotes: "notes",
  hotkeyNext: "next",
  hotkeyPrev: "prev",
};

export function SettingsView({
  settings,
  accounts,
  hotkeyFailures,
  onChange,
  onConnect,
  onToggleAccount,
  onDisconnect,
  onReconnect,
}: {
  settings: NotchSettings;
  accounts: CalendarAccount[];
  hotkeyFailures: HotkeyFailures;
  onChange: (settings: NotchSettings) => void;
  onConnect: (provider: CalendarProvider) => void;
  onToggleAccount: (id: string) => void;
  onDisconnect: (id: string) => void;
  onReconnect: (id: string) => void;
}) {
  const [connectOpen, setConnectOpen] = useState(false);
  const [captureErrors, setCaptureErrors] = useState<
    Partial<Record<ShortcutField | "shortcut", string>>
  >({});
  const validation = validateHotkeys(settings);
  const rows: { key: keyof NotchSettings; label: string; demo?: boolean }[] = [
    { key: "topEdge", label: "Open at top edge" },
    { key: "altAlt", label: "Open with Alt Alt" },
    { key: "hideInFullscreen", label: "Hide in fullscreen apps" },
    { key: "launchAtStartup", label: "Launch at startup" },
    { key: "upcomingEvent", label: "Demo: upcoming event", demo: true },
    { key: "noCalendars", label: "Demo: no calendars connected", demo: true },
  ];
  const compactRows: {
    key: "compactMusic" | "compactTimer" | "compactRecording" | "compactCharging";
    label: string;
  }[] = [
    { key: "compactMusic", label: "Music" },
    { key: "compactTimer", label: "Timer countdown" },
    { key: "compactRecording", label: "Recording" },
    { key: "compactCharging", label: "Charging" },
  ];

  const setCaptureError = (field: ShortcutField | "shortcut", message: string | null) => {
    setCaptureErrors((current) => ({ ...current, [field]: message ?? undefined }));
  };

  const captureHotkey = (field: ShortcutField, value: string) => {
    const next = { ...settings, [field]: value };
    const error = validateHotkeys(next)[field];
    if (error) {
      setCaptureError(field, error);
      return;
    }
    setCaptureError(field, null);
    onChange(next);
  };

  const captureDoubleAlt = (value: string) => {
    if (shortcutConflictsWithHotkeys(value, settings)) {
      setCaptureError("shortcut", "Shortcut already used");
      return;
    }
    setCaptureError("shortcut", null);
    onChange({ ...settings, shortcut: value });
  };

  const hotkeyError = (field: ShortcutField): string | null =>
    hotkeyFailures[FAILURE_KEY[field]] ?? captureErrors[field] ?? validation[field] ?? null;

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
                    <ShortcutInput
                      label="Open shortcut"
                      value={settings.shortcut}
                      error={captureErrors.shortcut}
                      onChange={captureDoubleAlt}
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

        <p className="mb-1.5 mt-3 text-[11px] font-semibold uppercase tracking-[.08em] text-white/40">
          Shortcuts
        </p>
        <div className="overflow-hidden rounded-[14px] border border-[#1f1f1f] bg-[#121212]">
          <div className="flex h-8 items-center justify-between border-b border-white/[0.06] px-2.5">
            <span
              className={`text-[11px] transition-colors ${
                settings.hotkeysEnabled ? "text-white" : "text-white/50"
              }`}
            >
              Global shortcuts
            </span>
            <Toggle
              value={settings.hotkeysEnabled}
              onChange={() => onChange({ ...settings, hotkeysEnabled: !settings.hotkeysEnabled })}
            />
          </div>
          {HOTKEY_ROWS.map(({ field, label }) => {
            const error = hotkeyError(field);
            return (
              <div
                key={field}
                className={`flex min-h-8 items-center justify-between gap-2 border-b border-white/[0.06] px-2.5 py-1 last:border-0 ${
                  settings.hotkeysEnabled ? "" : "opacity-50"
                }`}
              >
                <div className="min-w-0">
                  <span className="block text-[11px] text-white">{label}</span>
                  {error && <span className="block text-[10px] text-[#ff453a]">{error}</span>}
                </div>
                <ShortcutInput
                  label={`${label} shortcut`}
                  value={settings[field]}
                  error={error}
                  onChange={(value) => captureHotkey(field, value)}
                />
              </div>
            );
          })}
        </div>
        <p className="mb-1.5 mt-1.5 text-[10px] text-white/35">
          Click a shortcut, then press a modifier and key. Held keys fire once.
        </p>

        <p className="mb-1.5 mt-3 text-[11px] font-semibold uppercase tracking-[.08em] text-white/40">
          Compact bar
        </p>
        <p className="mb-1.5 text-[11px] text-white/50">
          Turn a row off to keep it out of the bar.
        </p>
        <div className="overflow-hidden rounded-[14px] border border-[#1f1f1f] bg-[#121212]">
          <div className="flex h-8 items-center justify-between border-b border-white/[0.06] px-2.5">
            <span
              className={`text-[11px] transition-colors ${
                settings.compactBar ? "text-white" : "text-white/50"
              }`}
            >
              Show compact bar
            </span>
            <Toggle
              value={settings.compactBar}
              onChange={() => onChange({ ...settings, compactBar: !settings.compactBar })}
            />
          </div>
          {compactRows.map(({ key, label }) => (
            <div
              key={key}
              className={`flex h-8 items-center justify-between border-b border-white/[0.06] pl-6 pr-2.5 last:border-0 ${
                settings.compactBar ? "" : "opacity-50"
              }`}
            >
              <span
                className={`text-[11px] transition-colors ${
                  settings.compactBar && settings[key] ? "text-white" : "text-white/50"
                }`}
              >
                {label}
              </span>
              <Toggle
                value={settings.compactBar && settings[key]}
                onChange={() => onChange({ ...settings, [key]: !settings[key] })}
              />
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
