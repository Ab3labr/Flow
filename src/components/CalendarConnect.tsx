import { Check, Eye, EyeOff, RefreshCw, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { CalendarAccount, CalendarProvider, calendarService } from "../services";
import { PressButton } from "./TopBar";

export function CalendarAccountRow({
  account,
  onToggle,
  onDisconnect,
  onReconnect,
}: {
  account: CalendarAccount;
  onToggle: () => void;
  onDisconnect: () => void;
  onReconnect: () => void;
}) {
  return (
    <div className="rounded-xl bg-[#121212] px-2.5 py-2">
      <div className="flex items-center gap-2">
        <span
          className="h-2.5 w-2.5 shrink-0 rounded-full"
          style={{ backgroundColor: account.color }}
        />
        <div className="min-w-0 flex-1">
          <p className="truncate text-[11px] font-medium text-white/80">{account.email}</p>
          <p
            className={`text-[11px] ${account.status === "error" ? "text-[#ff453a]" : "text-white/35"}`}
          >
            {account.status === "error"
              ? `Couldn't sync ${account.name.split(" ")[0]}. Reconnect`
              : "Synced just now"}
          </p>
        </div>
        <PressButton
          aria-label={account.visible ? "Hide calendar" : "Show calendar"}
          onClick={onToggle}
          className="grid h-6 w-6 place-items-center rounded-full text-white/40 hover:bg-white/[0.06] hover:text-white"
        >
          {account.visible ? <Eye className="h-3.5 w-3.5" /> : <EyeOff className="h-3.5 w-3.5" />}
        </PressButton>
        <PressButton
          aria-label="Refresh calendar"
          onClick={onReconnect}
          className="grid h-6 w-6 place-items-center rounded-full text-white/40 hover:bg-white/[0.06] hover:text-white"
        >
          <RefreshCw className="h-3.5 w-3.5" />
        </PressButton>
      </div>
      <div className="mt-1 flex justify-end gap-2">
        {account.status === "error" && (
          <button onClick={onReconnect} className="text-[11px] text-white/65 hover:text-white">
            Reconnect
          </button>
        )}
        <button onClick={onDisconnect} className="text-[11px] text-white/35 hover:text-white/70">
          Disconnect
        </button>
      </div>
    </div>
  );
}

export function CalendarConnectSheet({
  open,
  accountCount,
  onClose,
  onConnected,
}: {
  open: boolean;
  accountCount: number;
  onClose: () => void;
  onConnected: (provider: CalendarProvider) => void;
}) {
  const [provider, setProvider] = useState<CalendarProvider | null>(null);
  const [step, setStep] = useState<"providers" | "signin" | "connecting" | "connected">(
    "providers",
  );
  const providerInfo = calendarService.getProviders().find((item) => item.id === provider);

  useEffect(() => {
    if (!open) {
      setProvider(null);
      setStep("providers");
    }
  }, [open]);

  const connect = () => {
    if (!provider) return;
    setStep("connecting");
    window.setTimeout(() => {
      onConnected(provider);
      setStep("connected");
    }, 850);
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0, y: 8, filter: "blur(5px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          exit={{ opacity: 0, y: 6, filter: "blur(5px)" }}
          className="absolute inset-0 z-30 rounded-[14px] border border-[#1f1f1f] bg-[#121212] p-3"
        >
          <div className="flex h-6 items-center justify-between">
            <p className="text-[13px] font-semibold text-white">Connect calendar</p>
            <PressButton
              onClick={onClose}
              className="grid h-6 w-6 place-items-center rounded-full text-white/45 hover:bg-white/[0.06] hover:text-white"
            >
              <X className="h-3.5 w-3.5" />
            </PressButton>
          </div>

          {step === "providers" && (
            <div className="mt-1 space-y-0.5">
              {calendarService.getProviders().map((item) => (
                <button
                  key={item.id}
                  onClick={() => {
                    setProvider(item.id);
                    setStep("signin");
                  }}
                  className="flex h-7 w-full items-center gap-2 rounded-lg px-2 text-left text-[11px] text-white/70 hover:bg-white/[0.06] hover:text-white"
                >
                  <span className="grid h-5 w-5 place-items-center rounded-full bg-[#1c1c1c] text-[11px] font-semibold text-white/60">
                    {item.id === "outlook"
                      ? "M"
                      : item.id === "icloud"
                        ? "A"
                        : item.id === "ics"
                          ? "I"
                          : "G"}
                  </span>
                  {item.label}
                </button>
              ))}
            </div>
          )}

          {step === "signin" && providerInfo && (
            <div className="flex h-[130px] flex-col items-center justify-center text-center">
              <span className="grid h-9 w-9 place-items-center rounded-full bg-[#1c1c1c] text-[13px] font-semibold text-white/70">
                {provider === "outlook"
                  ? "M"
                  : provider === "icloud"
                    ? "A"
                    : provider === "ics"
                      ? "I"
                      : "G"}
              </span>
              <p className="mt-2 text-[11px] text-white/45">{providerInfo.label}</p>
              <PressButton
                onClick={connect}
                className="mt-3 h-8 rounded-full bg-white px-4 text-[11px] font-semibold text-black hover:bg-white/85"
              >
                {providerInfo.action}
              </PressButton>
            </div>
          )}

          {step === "connecting" && (
            <div className="flex h-[130px] items-center justify-center gap-2 text-[12px] text-white/65">
              <motion.span
                className="h-3.5 w-3.5 rounded-full border-2 border-white/20 border-t-white"
                animate={{ rotate: 360 }}
                transition={{ duration: 0.8, repeat: Infinity, ease: "linear" }}
              />
              Connecting…
            </div>
          )}

          {step === "connected" && (
            <div className="flex h-[130px] flex-col items-center justify-center">
              <span className="grid h-9 w-9 place-items-center rounded-full bg-white text-black">
                <Check className="h-4 w-4" />
              </span>
              <p className="mt-2 text-[12px] font-medium text-white">Calendar connected</p>
              <p className="text-[11px] text-white/40">{accountCount} accounts available</p>
              <PressButton
                onClick={onClose}
                className="mt-3 h-7 rounded-full bg-[#1c1c1c] px-4 text-[11px] text-white/70 hover:bg-white/15"
              >
                Done
              </PressButton>
            </div>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
