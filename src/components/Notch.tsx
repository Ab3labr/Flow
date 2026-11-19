import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { Timer as TimerIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  CalendarAccount,
  CalendarEvent,
  CalendarProvider,
  DEFAULT_NOTCH_SETTINGS,
  calendarService,
  mediaService,
  settingsService,
  type NotchSettings,
  type Track,
} from "../services";
import { isMockMode, isTauriApp } from "../services/env";
import {
  focusNotchWindow,
  onCloseRequest,
  onOpenRequest,
  setUiMode,
  type UiMode,
} from "../services/uiService";
import { CalendarView } from "./CalendarView";
import { CaptureView } from "./CaptureView";
import { EventCard, JoinButton } from "./EventCard";
import { MusicCard } from "./MusicCard";
import { NotesView } from "./NotesView";
import { SettingsView } from "./SettingsView";
import { TimerView } from "./TimerView";
import { PressButton, TabId, TopBar } from "./TopBar";

const baseHeights: Record<Exclude<TabId, "notes">, number> = {
  home: 172,
  timer: 250,
  calendar: 230,
  capture: 185,
  settings: 340,
};

const isTabId = (value: string): value is TabId =>
  (["home", "timer", "calendar", "notes", "capture", "settings"] as const).includes(
    value as TabId,
  );

/** Real tracks may have an unknown duration (`durationMs: null`). */
function trackSeconds(track: Track): number {
  if (track.durationMs !== undefined) {
    return track.durationMs == null ? 0 : track.durationMs / 1000;
  }
  return track.duration;
}

function IdleHome({
  onTimer,
  onNote,
  onSnip,
}: {
  onTimer: () => void;
  onNote: () => void;
  onSnip: () => void;
}) {
  const [now, setNow] = useState(new Date());

  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(id);
  }, []);

  const actions = [
    { label: "Start timer", action: onTimer },
    { label: "New note", action: onNote },
    { label: "Snip", action: onSnip },
  ];

  return (
    <div className="flex h-[120px] flex-col items-center justify-center rounded-[14px] bg-[#121212]">
      <p className="text-[30px] font-semibold leading-8 tracking-[-.04em] tabular-nums text-white">
        {now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
      </p>
      <p className="mt-1 text-[12px] text-white/45">
        {now.toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" })}
      </p>
      <div className="mt-2.5 flex gap-2">
        {actions.map(({ label, action }) => (
          <PressButton
            key={label}
            onClick={action}
            className="flex h-6 items-center rounded-full bg-[#1c1c1c] px-2.5 text-[11px] text-white/65 hover:bg-white/15 hover:text-white"
          >
            {label}
          </PressButton>
        ))}
      </div>
    </div>
  );
}

export function Notch() {
  const reducedMotion = useReducedMotion();
  const [expanded, setExpanded] = useState(false);
  const [pinned, setPinned] = useState(false);
  const [tab, setTab] = useState<TabId>("home");
  const mockTracks = useMemo(() => (isMockMode ? mediaService.getTracks() : []), []);
  const [trackIndex, setTrackIndex] = useState(0);
  const [playing, setPlaying] = useState(true);
  const [progress, setProgress] = useState(108);
  const [scrubbing, setScrubbing] = useState(false);
  const [realTrack, setRealTrack] = useState<Track | null>(null);
  const [mediaReady, setMediaReady] = useState(isMockMode);
  const [volume, setVolume] = useState(68);
  const [timerTotal, setTimerTotal] = useState(300);
  const [timerRemaining, setTimerRemaining] = useState(300);
  const [timerRunning, setTimerRunning] = useState(false);
  const [timerComplete, setTimerComplete] = useState(false);
  const [recording, setRecording] = useState(false);
  const [recordingTime, setRecordingTime] = useState(0);
  const [notesHeight, setNotesHeight] = useState(172);
  const [newNoteRequest, setNewNoteRequest] = useState(0);
  const [accounts, setAccounts] = useState<CalendarAccount[]>(calendarService.getDefaultAccounts);
  const [reminderEvent, setReminderEvent] = useState<CalendarEvent | null>(null);
  const [settings, setSettings] = useState<NotchSettings>(DEFAULT_NOTCH_SETTINGS);
  const hoverTimer = useRef<number | undefined>(undefined);
  const leaveTimer = useRef<number | undefined>(undefined);
  const focusTimer = useRef<number | undefined>(undefined);
  const completionTimer = useRef<number | undefined>(undefined);
  const reminderTimer = useRef<number | undefined>(undefined);
  const remindedEvent = useRef<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const expandedRef = useRef(false);
  const settingsRef = useRef<NotchSettings>(DEFAULT_NOTCH_SETTINGS);
  const settingsSaveSequence = useRef(0);
  const track = isMockMode ? mockTracks[trackIndex] : realTrack;
  const visibleAccounts = settings.noCalendars ? [] : accounts;
  const events = useMemo(
    () => calendarService.getToday(visibleAccounts),
    [accounts, settings.noCalendars],
  );
  const upcomingEvent = settings.upcomingEvent ? events[0] : undefined;
  const livePill = recording || timerComplete || Boolean(reminderEvent);
  const panelHeight = tab === "notes" ? notesHeight : baseHeights[tab];
  const uiMode: UiMode = expanded ? "open" : livePill ? "pill" : "idle";

  useEffect(() => {
    expandedRef.current = expanded;
  }, [expanded]);

  useEffect(() => {
    let cancelled = false;
    void settingsService
      .load()
      .then((loaded) => {
        if (cancelled) return;
        settingsRef.current = loaded;
        setSettings(loaded);
      })
      .catch((error) => {
        console.error("failed to load settings", error);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Real now-playing: `media-state` (state changes only) + the initial
  // `media_snapshot`. Null is the idle home; no fake tracks in real mode.
  useEffect(() => {
    if (isMockMode) return;
    let disposed = false;
    let unsubscribe: (() => void) | undefined;

    const apply = (next: Track | null) => {
      if (disposed) return;
      setRealTrack(next);
      setPlaying(next?.playing ?? false);
      setProgress(next?.positionMs != null ? next.positionMs / 1000 : 0);
      setMediaReady(true);
    };

    void mediaService
      .subscribe(apply)
      .then((stop) => {
        if (disposed) stop();
        else unsubscribe = stop;
      })
      .catch((error) => {
        console.error("media subscribe failed", error);
        if (!disposed) setMediaReady(true);
      });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  // Paint the new value immediately, then persist; revert if the write fails.
  const updateSettings = useCallback((next: NotchSettings) => {
    const previous = settingsRef.current;
    settingsRef.current = next;
    setSettings(next);
    const sequence = ++settingsSaveSequence.current;
    void settingsService.save(next).catch((error) => {
      console.error("failed to save settings", error);
      if (settingsSaveSequence.current !== sequence) return;
      settingsRef.current = previous;
      setSettings(previous);
    });
  }, []);

  // Report the window model to Rust. Open/pill resizes apply before Motion
  // expands; idle is only sent after the close animation has finished.
  useEffect(() => {
    if (uiMode !== "idle") {
      setUiMode(uiMode, panelHeight, pinned);
      return;
    }
    const id = window.setTimeout(() => setUiMode("idle", panelHeight, false), 250);
    return () => window.clearTimeout(id);
  }, [uiMode, panelHeight, pinned]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onOpenRequest((request) => {
      window.clearTimeout(hoverTimer.current);
      window.clearTimeout(leaveTimer.current);
      // Rust decides pinning: hover is unpinned, keyboard/tray are pinned.
      setPinned(request.pinned ?? request.source !== "hover");
      setExpanded(true);
      if (request.tab && isTabId(request.tab)) setTab(request.tab);
      if (request.source === "alt") {
        // Best effort: Esc works without focus because Rust owns the hook.
        window.clearTimeout(focusTimer.current);
        focusTimer.current = window.setTimeout(focusNotchWindow, 80);
      }
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const close = useCallback(() => {
    window.clearTimeout(focusTimer.current);
    setPinned(false);
    setExpanded(false);
  }, []);

  // Rust is the source of truth: double-Alt, Esc and outside clicks arrive as
  // close-request. The frontend obeys and clears the pin (close does both).
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onCloseRequest(() => {
      close();
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
      window.clearTimeout(focusTimer.current);
    };
  }, [close]);

  // Mock playback advances locally (browser prototype).
  useEffect(() => {
    if (!isMockMode || !playing || !track || scrubbing) return;
    const duration = trackSeconds(track);
    const id = window.setInterval(() => {
      setProgress((current) => {
        if (duration > 0 && current + 0.1 >= duration) {
          setTrackIndex((index) =>
            mockTracks.length ? (index + 1) % mockTracks.length : 0,
          );
          return 0;
        }
        return current + 0.1;
      });
    }, 100);
    return () => window.clearInterval(id);
  }, [playing, track, mockTracks.length, scrubbing]);

  // Real playback: interpolate from the last state change (positionMs is only
  // sent when the track/status changes, not on every poll tick).
  useEffect(() => {
    if (isMockMode || !realTrack?.playing || scrubbing) return;
    const base = realTrack.positionMs ?? 0;
    const capturedAt = realTrack.capturedAtMs ?? Date.now();
    const rate = realTrack.playbackRate ?? 1;
    const duration = trackSeconds(realTrack);
    const id = window.setInterval(() => {
      const next = (base + (Date.now() - capturedAt) * rate) / 1000;
      setProgress(duration > 0 ? Math.min(next, duration) : next);
    }, 100);
    return () => window.clearInterval(id);
  }, [realTrack, scrubbing]);

  useEffect(() => {
    if (!timerRunning) return;
    const id = window.setInterval(() => {
      setTimerRemaining((current) => {
        if (current <= 1) {
          setTimerRunning(false);
          setTimerComplete(true);
          window.clearTimeout(completionTimer.current);
          completionTimer.current = window.setTimeout(() => setTimerComplete(false), 3000);
          return 0;
        }
        return current - 1;
      });
    }, 1000);
    return () => window.clearInterval(id);
  }, [timerRunning]);

  useEffect(() => {
    if (!recording) return;
    const id = window.setInterval(() => setRecordingTime((time) => time + 1), 1000);
    return () => window.clearInterval(id);
  }, [recording]);

  useEffect(() => {
    const event = events.find((item) => {
      const until = item.startsAt - Date.now();
      return until > 0 && until <= 5 * 60_000 && calendarService.getMeetingProvider(item);
    });
    if (!event || remindedEvent.current === event.id) return;
    remindedEvent.current = event.id;
    const showTimer = window.setTimeout(() => {
      setReminderEvent(event);
      reminderTimer.current = window.setTimeout(() => setReminderEvent(null), 6000);
    }, 900);
    return () => window.clearTimeout(showTimer);
  }, [events]);

  useEffect(() => {
    let lastAlt = 0;
    const toggleFromKeyboard = () => {
      if (expandedRef.current) {
        close();
      } else {
        window.clearTimeout(leaveTimer.current);
        setExpanded(true);
        setPinned(true);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Alt") {
        // The real app uses the Rust WH_KEYBOARD_LL hook; the JS sequence is
        // only for the browser prototype.
        if (isTauriApp) return;
        event.preventDefault();
        if (event.repeat || !settings.altAlt || settings.shortcut !== "Alt Alt") return;
        const now = Date.now();
        if (now - lastAlt <= 350) {
          toggleFromKeyboard();
          lastAlt = 0;
        } else {
          lastAlt = now;
        }
        return;
      }
      lastAlt = 0;
      if (event.key === "Escape") close();
      if (!settings.altAlt || settings.shortcut === "Alt Alt" || event.repeat) return;
      const key =
        event.key === " "
          ? "Space"
          : event.key.length === 1
            ? event.key.toUpperCase()
            : event.key.replace("Control", "Ctrl");
      const pressed = [
        event.ctrlKey && key !== "Ctrl" ? "Ctrl" : "",
        event.altKey && key !== "Alt" ? "Alt" : "",
        event.shiftKey && key !== "Shift" ? "Shift" : "",
        event.metaKey && key !== "Meta" ? "Meta" : "",
        key,
      ]
        .filter(Boolean)
        .join(" + ");
      if (pressed === settings.shortcut) {
        event.preventDefault();
        toggleFromKeyboard();
      }
    };
    const onKeyUp = (event: KeyboardEvent) => {
      if (event.key === "Alt") event.preventDefault();
    };
    const onPointer = (event: PointerEvent) => {
      if (
        expandedRef.current &&
        panelRef.current &&
        !panelRef.current.contains(event.target as Node)
      )
        close();
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("pointerdown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      window.removeEventListener("pointerdown", onPointer);
    };
  }, [close, settings.altAlt, settings.shortcut]);

  const openFromEdge = () => {
    // The real app opens from the Rust hot-zone poll; this 1 px strip only
    // drives the browser prototype.
    if (isTauriApp) return;
    window.clearTimeout(leaveTimer.current);
    window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => setExpanded(true), 180);
  };

  const enterPanel = () => {
    window.clearTimeout(leaveTimer.current);
    window.clearTimeout(hoverTimer.current);
    if (reminderEvent) window.clearTimeout(reminderTimer.current);
    if (livePill && !expanded) setExpanded(true);
  };

  const leaveTarget = () => {
    window.clearTimeout(hoverTimer.current);
    if (!pinned) leaveTimer.current = window.setTimeout(() => setExpanded(false), 400);
    if (reminderEvent) {
      window.clearTimeout(reminderTimer.current);
      reminderTimer.current = window.setTimeout(() => setReminderEvent(null), 6000);
    }
  };

  const skip = (direction: number) => {
    if (isMockMode) {
      if (mockTracks.length === 0) return;
      setTrackIndex((index) => (index + direction + mockTracks.length) % mockTracks.length);
      setProgress(0);
      return;
    }
    void (direction < 0 ? mediaService.prev() : mediaService.next());
  };

  const togglePlayback = (nextPlaying: boolean) => {
    // Optimistic; the next media-state is authoritative.
    setPlaying(nextPlaying);
    if (!isMockMode) void mediaService.toggle();
  };

  const scrub = (value: number) => {
    setScrubbing(true);
    setProgress(value);
  };

  const commitScrub = (value: number) => {
    setScrubbing(false);
    setProgress(value);
    if (isMockMode) return;
    // Keep interpolating from the dropped handle until the next event arrives.
    setRealTrack((current) =>
      current
        ? {
            ...current,
            positionMs: Math.round(value * 1000),
            capturedAtMs: Date.now(),
          }
        : current,
    );
    void mediaService.seek(Math.round(value * 1000));
  };

  const setPreset = (minutes: number) => {
    const seconds = minutes * 60;
    setTimerTotal(seconds);
    setTimerRemaining(seconds);
    setTimerRunning(false);
    setTimerComplete(false);
    window.clearTimeout(completionTimer.current);
  };

  const openNewNote = () => {
    setTab("notes");
    setNewNoteRequest((request) => request + 1);
  };

  const connectCalendar = (provider: CalendarProvider) => {
    setAccounts((current) => {
      const account = calendarService.createAccount(
        provider,
        settings.noCalendars ? 0 : current.length,
      );
      return settings.noCalendars ? [account] : [...current, account];
    });
    updateSettings({ ...settingsRef.current, noCalendars: false });
  };

  const toggleCalendar = (id: string) => {
    setAccounts((current) =>
      current.map((account) =>
        account.id === id ? { ...account, visible: !account.visible } : account,
      ),
    );
  };

  const reconnectCalendar = (id: string) => {
    setAccounts((current) =>
      current.map((account) =>
        account.id === id ? { ...account, status: "synced" as const } : account,
      ),
    );
  };

  const showIdleHome = isMockMode ? !playing && !upcomingEvent : !mediaReady || !realTrack;

  const home = showIdleHome ? (
    <IdleHome
      onTimer={() => setTab("timer")}
      onNote={openNewNote}
      onSnip={() => setTab("capture")}
    />
  ) : (
    <motion.div layout className="flex h-[120px] gap-2 px-1">
      {track && (
        <MusicCard
          track={track}
          playing={playing}
          progress={progress}
          volume={volume}
          onPlayingChange={togglePlayback}
          onSkip={skip}
          onScrub={scrub}
          onScrubCommit={commitScrub}
          onVolumeChange={setVolume}
        />
      )}
      <AnimatePresence initial={false}>
        {upcomingEvent && (
          <motion.div
            layout
            className="overflow-hidden"
            initial={{ opacity: 0, width: 0, filter: "blur(5px)" }}
            animate={{ opacity: 1, width: 148, filter: "blur(0px)" }}
            exit={{ opacity: 0, width: 0, filter: "blur(5px)" }}
            transition={{ type: "spring", stiffness: 380, damping: 30, mass: 0.9 }}
          >
            <EventCard event={upcomingEvent} eventCount={events.length} />
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );

  const content: Record<TabId, React.ReactNode> = {
    home,
    timer: (
      <TimerView
        total={timerTotal}
        remaining={timerRemaining}
        running={timerRunning}
        complete={timerComplete}
        onPreset={setPreset}
        onAddTime={(seconds) => {
          setTimerRemaining((remaining) => remaining + seconds);
          setTimerTotal((total) => total + seconds);
        }}
        onToggle={() => {
          if (timerRemaining === 0) setTimerRemaining(timerTotal);
          setTimerRunning((running) => !running);
        }}
      />
    ),
    calendar: (
      <CalendarView
        accounts={visibleAccounts}
        events={events}
        onConnect={connectCalendar}
        onReconnect={reconnectCalendar}
      />
    ),
    notes: (
      <NotesView
        newNoteRequest={newNoteRequest}
        onHeightChange={(height) => setNotesHeight(Math.max(172, Math.min(340, height)))}
        onRequestHandled={() => setNewNoteRequest(0)}
      />
    ),
    capture: (
      <CaptureView
        recording={recording}
        elapsed={recordingTime}
        onRecordingChange={(value) => {
          setRecording(value);
          if (value) setRecordingTime(0);
        }}
      />
    ),
    settings: (
      <SettingsView
        settings={settings}
        accounts={visibleAccounts}
        onChange={updateSettings}
        onConnect={connectCalendar}
        onToggleAccount={toggleCalendar}
        onDisconnect={(id) =>
          setAccounts((current) => current.filter((account) => account.id !== id))
        }
        onReconnect={reconnectCalendar}
      />
    ),
  };

  const recordingDisplay = `${Math.floor(recordingTime / 60)}:${String(recordingTime % 60).padStart(2, "0")}`;
  const reminderProvider = reminderEvent ? calendarService.getMeetingProvider(reminderEvent) : null;

  return (
    <div className="pointer-events-none fixed inset-x-0 top-0 z-50 h-0">
      {settings.topEdge && !expanded && !livePill && (
        <div
          aria-label="Open notch"
          className="pointer-events-auto absolute left-1/2 top-0 h-1 w-60 -translate-x-1/2"
          onPointerEnter={openFromEdge}
          onPointerLeave={leaveTarget}
        />
      )}
      <motion.div
        ref={panelRef}
        role={expanded ? "dialog" : undefined}
        aria-label={expanded ? "Windows notch utility" : undefined}
        className={`absolute left-1/2 top-0 -translate-x-1/2 overflow-hidden bg-black ${
          expanded || livePill ? "pointer-events-auto" : "pointer-events-none"
        } ${expanded || livePill ? "shadow-[0_12px_34px_rgba(0,0,0,.36)]" : ""}`}
        onPointerEnter={enterPanel}
        onPointerLeave={leaveTarget}
        animate={{
          width: expanded ? 480 : livePill ? 108 : 120,
          height: expanded ? panelHeight : livePill ? 26 : 4,
          opacity: expanded || livePill ? 1 : 0,
          borderBottomLeftRadius: expanded ? 22 : livePill ? 13 : 2,
          borderBottomRightRadius: expanded ? 22 : livePill ? 13 : 2,
        }}
        transition={
          reducedMotion
            ? { duration: 0.15 }
            : expanded
              ? { type: "spring", stiffness: 380, damping: 30, mass: 0.9 }
              : {
                  height: { delay: 0.1, duration: 0.22 },
                  width: { delay: 0.1, duration: 0.22 },
                  opacity: { delay: livePill ? 0 : 0.18, duration: 0.15 },
                }
        }
      >
        <AnimatePresence mode="wait">
          {expanded ? (
            <motion.div
              key="panel"
              className="h-full w-full px-2.5 pt-2.5"
              initial={{ opacity: 0, filter: "blur(6px)", y: 6 }}
              animate={{ opacity: 1, filter: "blur(0px)", y: 0 }}
              exit={{ opacity: 0, filter: "blur(6px)", transition: { duration: 0.12 } }}
              transition={{
                delay: reducedMotion ? 0 : 0.13,
                duration: reducedMotion ? 0.15 : 0.24,
                ease: [0.22, 1, 0.36, 1],
              }}
              onClick={(event) => {
                event.stopPropagation();
                if (!(event.target as HTMLElement).closest("button, input, textarea"))
                  setPinned(true);
              }}
            >
              <TopBar active={tab} onChange={setTab} />
              <div className="mt-1.5">
                <AnimatePresence mode="wait">
                  <motion.div
                    key={tab}
                    initial={{ opacity: 0, filter: "blur(6px)", y: 8 }}
                    animate={{ opacity: 1, filter: "blur(0px)", y: 0 }}
                    exit={{ opacity: 0, filter: "blur(5px)", transition: { duration: 0.12 } }}
                    transition={{
                      duration: reducedMotion ? 0.15 : 0.22,
                      ease: [0.22, 1, 0.36, 1],
                    }}
                  >
                    {content[tab]}
                  </motion.div>
                </AnimatePresence>
              </div>
            </motion.div>
          ) : livePill ? (
            <motion.div
              key={recording ? "recording" : "timer-done"}
              initial={{ opacity: 0, y: -5 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -5 }}
              className="flex h-[26px] items-center justify-center gap-2 px-2.5"
            >
              {recording ? (
                <>
                  <span className="relative flex h-2.5 w-2.5">
                    <motion.span
                      className="absolute inset-0 rounded-full bg-[#ff453a]"
                      animate={{ opacity: [0.55, 0, 0.55], scale: [1, 2, 1] }}
                      transition={{ duration: 1.8, repeat: Infinity }}
                    />
                    <span className="relative h-2.5 w-2.5 rounded-full bg-[#ff453a]" />
                  </span>
                  <span className="text-[11px] font-medium tabular-nums text-white/70">
                    {recordingDisplay}
                  </span>
                </>
              ) : timerComplete ? (
                <>
                  <motion.span
                    animate={{ scale: [1, 1.35, 1], opacity: [0.5, 1, 0.5] }}
                    transition={{ duration: 1.5, repeat: Infinity }}
                  >
                    <TimerIcon className="h-3.5 w-3.5 text-white/70" />
                  </motion.span>
                  <span className="text-[11px] font-medium text-white/70">Timer done</span>
                </>
              ) : reminderEvent ? (
                <>
                  <span className="min-w-0 flex-1 truncate text-[11px] font-medium text-white/70">
                    {reminderEvent.title} in 5 min
                  </span>
                  <JoinButton provider={reminderProvider} compact />
                </>
              ) : null}
            </motion.div>
          ) : null}
        </AnimatePresence>
      </motion.div>
    </div>
  );
}
