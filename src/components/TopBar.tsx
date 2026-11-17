import { AnimatePresence, motion } from "motion/react";
import { CalendarDays, Camera, Home, NotebookPen, Settings, Timer } from "lucide-react";

export type TabId = "home" | "timer" | "calendar" | "notes" | "capture" | "settings";

const tabs = [
  { id: "home" as const, label: "Home", icon: Home },
  { id: "timer" as const, label: "Timer", icon: Timer },
  { id: "calendar" as const, label: "Calendar", icon: CalendarDays },
  { id: "notes" as const, label: "Notes", icon: NotebookPen },
  { id: "capture" as const, label: "Capture", icon: Camera },
];

export function PressButton({
  children,
  className = "",
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <motion.button
      whileTap={{ scale: 0.92 }}
      transition={{ type: "spring", stiffness: 600, damping: 30 }}
      className={className}
      {...(props as object)}
    >
      {children}
    </motion.button>
  );
}

export function TopBar({ active, onChange }: { active: TabId; onChange: (tab: TabId) => void }) {
  const tabButton = (id: TabId, label: string, Icon: typeof Home) => {
    const isActive = active === id;
    return (
      <PressButton
        key={id}
        type="button"
        aria-label={label}
        aria-pressed={isActive}
        onClick={(event) => {
          event.stopPropagation();
          onChange(id);
        }}
        className="relative grid h-[26px] w-[26px] place-items-center rounded-full text-white/60 hover:bg-white/[0.06] hover:text-white"
      >
        <AnimatePresence>
          {isActive && (
            <motion.span
              layoutId="active-tab"
              className="absolute inset-0 rounded-full bg-[#1c1c1c]"
              transition={{ type: "spring", stiffness: 420, damping: 32 }}
            />
          )}
        </AnimatePresence>
        <Icon className="relative z-10 h-[15px] w-[15px]" strokeWidth={1.8} />
      </PressButton>
    );
  };

  return (
    <div className="flex h-[26px] items-center justify-between">
      <div className="flex items-center gap-1">
        {tabs.map((tab) => tabButton(tab.id, tab.label, tab.icon))}
      </div>
      {tabButton("settings", "Settings", Settings)}
    </div>
  );
}
