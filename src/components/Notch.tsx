import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { CalendarDays, Square } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  CalendarAccount,
  CalendarProvider,
  DEFAULT_NOTCH_SETTINGS,
  calendarService,
  mediaService,
  powerService,
  settingsService,
  type NotchSettings,
  type Track,
} from "../services";
import { powerActivity, reducePowerEvent } from "../live/powerActivity";
import { debugLog } from "../services/debugLog";
import { isMockMode, isTauriApp } from "../services/env";
import {
  focusNotchWindow,
  isFullscreenAppActive,
  onCloseRequest,
  onOpenRequest,
  uiReady,
} from "../services/uiService";
import {
  COMPACT_BAR_HEIGHT,
  COMPACT_BAR_MAX_WIDTH,
  COMPACT_BAR_MIN_WIDTH,
  CompactBar,
} from "../live/CompactBar";
import { IDLE_BAR_HEIGHT, IDLE_BAR_WIDTH } from "../live/geometry";
import { LIVE_PRIORITIES } from "../live/priorities";
import type { LiveActivity } from "../live/types";
import { useLiveActivities } from "../live/useLiveActivities";
import { useWindowTransition } from "../live/useWindowTransition";
import type { Surface, WindowTarget } from "../live/windowTransition";
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
  (["home", "timer", "calendar", "notes", "capture", "settings"] as const).includes(value as TabId);

const SURFACE_SPRING = { type: "spring", stiffness: 380, damping: 30, mass: 0.9 } as const;
const CONTENT_EASE = [0.22, 1, 0.36, 1] as const;

/** Real tracks may have an unknown duration (`durationMs: null`). */
function trackSeconds(track: Track): number {
  if (track.durationMs !== undefined) {
    return track.durationMs == null ? 0 : track.durationMs / 1000;
  }
  return track.duration;
}

/** Recording: `MM:SS`, or `H:MM:SS` once an hour has elapsed. */
function formatElapsed(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const remainder = total % 60;
  const tail = `${String(minutes).padStart(2, "0")}:${String(remainder).padStart(2, "0")}`;
  return hours > 0 ? `${hours}:${tail}` : tail;
}

/** Timer compact label: `M:SS` (no leading minute zero). */
function formatTimer(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

/** FLOAT_DEBUG=1 (Rust also sets window.FLOAT_DEBUG) enables stage timings. */
function powerDebugEnabled(): boolean {
  if (import.meta.env.VITE_FLOAT_DEBUG === "1") return true;
  const flag = (window as Window & { FLOAT_DEBUG?: unknown }).FLOAT_DEBUG;
  return flag === "1" || flag === true;
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
  const [timerActive, setTimerActive] = useState(false);
  const [timerComplete, setTimerComplete] = useState(false);
  const [recording, setRecording] = useState(false);
  const [recordingTime, setRecordingTime] = useState(0);
  const [notesHeight, setNotesHeight] = useState(172);
  const [newNoteRequest, setNewNoteRequest] = useState(0);
  const [accounts, setAccounts] = useState<CalendarAccount[]>(calendarService.getDefaultAccounts);
  const [meetingActivity, setMeetingActivity] = useState<LiveActivity | null>(null);
  const [settings, setSettings] = useState<NotchSettings>(DEFAULT_NOTCH_SETTINGS);
  const hoverTimer = useRef<number | undefined>(undefined);
  const leaveTimer = useRef<number | undefined>(undefined);
  const focusTimer = useRef<number | undefined>(undefined);
  const completionTimer = useRef<number | undefined>(undefined);
  const dissolveTimer = useRef<number | undefined>(undefined);
  const remindedEvent = useRef<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const expandedRef = useRef(false);
  const settingsRef = useRef<NotchSettings>(DEFAULT_NOTCH_SETTINGS);
  const settingsSaveSequence = useRef(0);
  const track = isMockMode ? mockTracks[trackIndex] : realTrack;
  const { top: topActivity, register, update, remove, clear } = useLiveActivities();
  const [barWidth, setBarWidth] = useState(COMPACT_BAR_MIN_WIDTH);
  const [fullscreenActive, setFullscreenActive] = useState(false);
  const musicRegistered = useRef(false);
  const musicRemoveTimer = useRef<number | undefined>(undefined);
  const powerHideTimer = useRef<number | undefined>(undefined);
  const powerVisible = useRef(false);
  const powerStage = useRef({ receivedAt: 0, registeredAt: 0, visibleLogged: false });
  const visibleAccounts = settings.noCalendars ? [] : accounts;
  const events = useMemo(
    () => calendarService.getToday(visibleAccounts),
    [accounts, settings.noCalendars],
  );
  const recordingDisplay = formatElapsed(recordingTime);
  const timerDisplay = formatTimer(timerRemaining);
  const timerProgress = timerTotal > 0 ? timerRemaining / timerTotal : 0;
  const upcomingEvent = settings.upcomingEvent ? events[0] : undefined;

  const musicActivity = useMemo<LiveActivity | null>(() => {
    if (!track || !track.isMusicApp || !settings.compactBar || !settings.compactMusic) return null;
    return {
      id: "music",
      kind: "music",
      mode: "persistent",
      priority: LIVE_PRIORITIES.music,
      leading: track.artDataUrl
        ? { type: "image", url: track.artDataUrl }
        : {
            type: "initial",
            text: track.source.trim().charAt(0).toUpperCase() || "?",
            colors: track.colors,
          },
      label: track.title,
      trailing: "equalizer",
      playing,
      hover: "expand",
      expandTab: "home",
    };
  }, [track, playing, settings.compactBar, settings.compactMusic]);

  const recordingActivity = useMemo<LiveActivity | null>(() => {
    if (!recording || !settings.compactBar || !settings.compactRecording) return null;
    return {
      id: "recording",
      kind: "recording",
      mode: "persistent",
      priority: LIVE_PRIORITIES.recording,
      accent: "#ff453a",
      leading: { type: "recordingDot", color: "#ff453a" },
      label: recordingDisplay,
      hover: "actions",
      expandTab: "capture",
      actions: [
        {
          id: "stop",
          label: "Stop recording",
          icon: (
            <span className="grid h-4 w-4 place-items-center rounded-full bg-white/15 text-white hover:bg-white/25">
              <Square className="h-2.5 w-2.5 fill-current" strokeWidth={2} />
            </span>
          ),
          onClick: () => setRecording(false),
        },
      ],
    };
  }, [recording, recordingDisplay, settings.compactBar, settings.compactRecording]);

  const timerActivity = useMemo<LiveActivity | null>(() => {
    if (!timerActive || timerComplete || !settings.compactBar || !settings.compactTimer)
      return null;
    return {
      id: "timer",
      kind: "timer",
      mode: "persistent",
      priority: LIVE_PRIORITIES.timer,
      leading: { type: "ring", progress: timerProgress },
      label: timerDisplay,
      trailing: timerRunning ? "none" : "pause",
      playing: timerRunning,
      hover: "expand",
      expandTab: "timer",
      maxWidth: 170,
    };
  }, [
    timerActive,
    timerComplete,
    timerProgress,
    timerDisplay,
    timerRunning,
    settings.compactBar,
    settings.compactTimer,
  ]);

  const timerDoneActivity = useMemo<LiveActivity | null>(() => {
    if (!timerComplete || !settings.compactBar || !settings.compactTimer) return null;
    return {
      id: "timer-done",
      kind: "timer-done",
      mode: "transient",
      ttlMs: 5000,
      priority: LIVE_PRIORITIES.timerDone,
      leading: { type: "pulse", name: "timer", color: "#f09ab7" },
      label: "Timer done",
      hover: "none",
    };
  }, [timerComplete, settings.compactBar, settings.compactTimer]);

  const compact = Boolean(
    settings.compactBar &&
    !expanded &&
    topActivity &&
    !(settings.hideInFullscreen && fullscreenActive),
  );
  const panelHeight = tab === "notes" ? notesHeight : baseHeights[tab];
  const surface: Surface = expanded ? "open" : compact ? "compact" : "idle";
  const windowTarget = useMemo<WindowTarget>(
    () => ({
      surface,
      geometry: { panelHeight, barWidth, barHeight: COMPACT_BAR_HEIGHT },
    }),
    [surface, panelHeight, barWidth],
  );
  const { transitioning, resend } = useWindowTransition(
    windowTarget,
    pinned,
    Boolean(reducedMotion),
  );

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

  // Music activity: only `isMusicApp` sessions register, and only while
  // playing; a pause keeps it (flat bars) for 3 s so the bar does not flicker
  // during a quick resume.
  useEffect(() => {
    window.clearTimeout(musicRemoveTimer.current);
    musicRemoveTimer.current = undefined;
    if (!musicActivity) {
      if (musicRegistered.current) remove("music");
      musicRegistered.current = false;
      return;
    }
    if (musicActivity.playing) {
      register(musicActivity);
      musicRegistered.current = true;
      return;
    }
    if (!musicRegistered.current) return;
    update("music", { ...musicActivity, playing: false });
    musicRemoveTimer.current = window.setTimeout(() => {
      remove("music");
      musicRegistered.current = false;
    }, 3000);
  }, [musicActivity, register, remove, update]);

  useEffect(
    () => () => {
      window.clearTimeout(musicRemoveTimer.current);
      window.clearTimeout(dissolveTimer.current);
      clear();
    },
    [clear],
  );

  useEffect(() => {
    if (!recordingActivity) {
      remove("recording");
      return;
    }
    register(recordingActivity);
  }, [recordingActivity, register, remove]);

  useEffect(() => {
    if (!timerActivity) {
      remove("timer");
      return;
    }
    register(timerActivity);
  }, [timerActivity, register, remove]);

  useEffect(() => {
    if (!timerDoneActivity) {
      remove("timer-done");
      return;
    }
    register(timerDoneActivity);
  }, [timerDoneActivity, register, remove]);

  useEffect(() => {
    if (!meetingActivity) {
      remove("meeting");
      return;
    }
    register(meetingActivity);
  }, [meetingActivity, register, remove]);

  // A transition starts a fresh 4 s slot; percent updates only refresh the
  // visible slot, so they never restart the store's TTL timer.
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;

    const stopPower = () => {
      powerVisible.current = false;
      window.clearTimeout(powerHideTimer.current);
      powerHideTimer.current = undefined;
      remove("charging");
    };

    if (!settings.compactBar || !settings.compactCharging) {
      stopPower();
      return;
    }

    void powerService
      .subscribe((state) => {
        if (disposed) return;
        const debug = powerDebugEnabled();

        if (debug) {
          powerStage.current.receivedAt = performance.now();
          console.info(
            `[power] stage 3 received t=${Math.round(powerStage.current.receivedAt)}ms transition=${state.transition}`,
          );
        }

        // A percent-only drain must not create the bar (BUG 2).
        const action = reducePowerEvent(state, powerVisible.current);
        if (action === "ignore") {
          if (debug) console.info("[power] ignored: no visible slot");
          return;
        }

        const activity = powerActivity(state);
        if (action === "register") {
          register(activity);
          powerVisible.current = true;
          powerStage.current.visibleLogged = false;
          window.clearTimeout(powerHideTimer.current);
          powerHideTimer.current = window.setTimeout(stopPower, 4_000);
          if (debug) {
            powerStage.current.registeredAt = performance.now();
            console.info(
              `[power] stage 4 registered delta=${Math.round(
                powerStage.current.registeredAt - powerStage.current.receivedAt,
              )}ms`,
            );
          }
          return;
        }

        update("charging", {
          accent: activity.accent,
          leading: activity.leading,
          label: activity.label,
          trailing: activity.trailing,
        });
        if (debug) console.info("[power] updated percent (timer unchanged)");
      })
      .then((unsubscribe) => {
        if (disposed) unsubscribe();
        else stop = unsubscribe;
      })
      .catch((error) => console.error("power subscribe failed", error));

    return () => {
      disposed = true;
      stop?.();
      stopPower();
    };
  }, [settings.compactBar, settings.compactCharging, register, remove, update]);

  // Debug-only: time from activity registration to the bar becoming visible.
  useEffect(() => {
    if (!powerDebugEnabled()) return;
    if (surface !== "compact" || topActivity?.id !== "charging") return;
    if (powerStage.current.visibleLogged || powerStage.current.registeredAt === 0) return;
    powerStage.current.visibleLogged = true;
    const delta = performance.now() - powerStage.current.registeredAt;
    console.info(
      `[power] stage 5 bar visible delta=${Math.round(delta)}ms${delta > 150 ? " (>150ms)" : ""}`,
    );
  }, [surface, topActivity]);

  // Fullscreen suppression mirrors the Rust hover trigger.
  useEffect(() => {
    if (!isTauriApp || !settings.compactBar || !settings.hideInFullscreen || expanded) {
      setFullscreenActive(false);
      return;
    }
    let cancelled = false;
    const check = () => {
      void isFullscreenAppActive().then((active) => {
        if (!cancelled) setFullscreenActive(active);
      });
    };
    check();
    const id = window.setInterval(check, 1000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [settings.compactBar, settings.hideInFullscreen, expanded]);

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
    window.clearTimeout(dissolveTimer.current);
    dissolveTimer.current = undefined;
    setPinned(false);
    setExpanded(false);
  }, []);

  /** Start/Record collapse the panel into the live bar after a short beat. */
  const collapseIntoBar = useCallback(() => {
    window.clearTimeout(dissolveTimer.current);
    dissolveTimer.current = window.setTimeout(() => close(), 450);
  }, [close]);

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
          setTrackIndex((index) => (mockTracks.length ? (index + 1) % mockTracks.length : 0));
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
          setTimerActive(false);
          setTimerComplete(true);
          window.clearTimeout(completionTimer.current);
          completionTimer.current = window.setTimeout(() => setTimerComplete(false), 5000);
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
    if (!settings.compactBar) return;
    const provider = calendarService.getMeetingProvider(event);
    let hideTimer: number | undefined;
    const showTimer = window.setTimeout(() => {
      setMeetingActivity({
        id: "meeting",
        kind: "meeting",
        mode: "transient",
        ttlMs: 6000,
        priority: LIVE_PRIORITIES.meetingReminder,
        accent: "#30d158",
        leading: { type: "icon", name: "meeting" },
        label: `${event.title} in 5 min`,
        hover: "none",
        expandTab: "calendar",
        render: ({ activity }) => (
          <>
            <span className="grid h-5 w-5 shrink-0 place-items-center rounded-[6px] bg-white/10">
              <CalendarDays className="h-3 w-3 text-white/80" strokeWidth={1.8} />
            </span>
            <span className="min-w-0 flex-1 truncate text-[11px] font-medium text-white">
              {activity.label}
            </span>
            <span onClick={(click) => click.stopPropagation()}>
              <JoinButton provider={provider ?? null} compact />
            </span>
          </>
        ),
      });
      hideTimer = window.setTimeout(() => setMeetingActivity(null), 6000);
    }, 900);
    return () => {
      window.clearTimeout(showTimer);
      if (hideTimer !== undefined) window.clearTimeout(hideTimer);
    };
  }, [events, settings.compactBar]);

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

  // Runs after every listener effect above. Rust stays closed and queues
  // triggers until this fires, then the first plan is force-sent so both
  // sides start from the same state (startup notch bug).
  useEffect(() => {
    let cancelled = false;
    debugLog(`[ui] ready effect mounted visibility=${document.visibilityState}`);
    void uiReady()
      .then(() => {
        if (!cancelled) resend();
      })
      .catch((error) => console.error("ui_ready failed", error));
    return () => {
      cancelled = true;
    };
  }, [resend]);

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
    if (compact && !expanded && topActivity?.hover === "expand") {
      setTab(topActivity.expandTab ?? "home");
      setExpanded(true);
    }
  };

  const openPinnedFromBar = () => {
    window.clearTimeout(hoverTimer.current);
    window.clearTimeout(leaveTimer.current);
    setTab(topActivity?.expandTab ?? "home");
    setPinned(true);
    setExpanded(true);
  };

  const barMaxWidth = topActivity?.maxWidth ?? COMPACT_BAR_MAX_WIDTH;
  const handleBarWidth = useCallback(
    (width: number) => {
      if (!compact) return;
      const next = Math.max(COMPACT_BAR_MIN_WIDTH, Math.min(barMaxWidth, width));
      setBarWidth((current) => (current === next ? current : next));
    },
    [barMaxWidth, compact],
  );

  const leaveTarget = () => {
    window.clearTimeout(hoverTimer.current);
    if (!pinned) leaveTimer.current = window.setTimeout(() => setExpanded(false), 400);
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
    setTimerActive(false);
    setTimerComplete(false);
    window.clearTimeout(completionTimer.current);
    window.clearTimeout(dissolveTimer.current);
    dissolveTimer.current = undefined;
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
          const starting = !timerRunning;
          if (timerRemaining === 0) setTimerRemaining(timerTotal);
          setTimerRunning(starting);
          if (starting) {
            setTimerActive(true);
            setTimerComplete(false);
            window.clearTimeout(completionTimer.current);
            if (settings.compactBar && settings.compactTimer) collapseIntoBar();
          } else {
            window.clearTimeout(dissolveTimer.current);
            dissolveTimer.current = undefined;
          }
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
          if (value) {
            setRecordingTime(0);
            if (settings.compactBar && settings.compactRecording) collapseIntoBar();
          } else {
            window.clearTimeout(dissolveTimer.current);
            dissolveTimer.current = undefined;
          }
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

  return (
    <div className="pointer-events-none fixed inset-x-0 top-0 z-50 h-0">
      {settings.topEdge && surface === "idle" && !transitioning && (
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
        className={`absolute left-1/2 top-0 -translate-x-1/2 overflow-visible bg-black ${
          surface === "idle" ? "pointer-events-none" : "pointer-events-auto"
        } ${surface === "idle" ? "" : "shadow-[0_12px_34px_rgba(0,0,0,.36)]"}`}
        onPointerEnter={enterPanel}
        onPointerLeave={leaveTarget}
        animate={{
          width: surface === "open" ? 480 : surface === "compact" ? barWidth : IDLE_BAR_WIDTH,
          height:
            surface === "open"
              ? panelHeight
              : surface === "compact"
                ? COMPACT_BAR_HEIGHT
                : IDLE_BAR_HEIGHT,
          opacity: surface === "idle" ? 0 : 1,
          borderBottomLeftRadius: surface === "open" ? 22 : surface === "compact" ? 15 : 2,
          borderBottomRightRadius: surface === "open" ? 22 : surface === "compact" ? 15 : 2,
        }}
        transition={{
          width: reducedMotion ? { duration: 0.15 } : SURFACE_SPRING,
          height: reducedMotion ? { duration: 0.15 } : SURFACE_SPRING,
          borderBottomLeftRadius: reducedMotion ? { duration: 0.15 } : SURFACE_SPRING,
          borderBottomRightRadius: reducedMotion ? { duration: 0.15 } : SURFACE_SPRING,
          opacity: reducedMotion ? { duration: 0.15 } : { duration: 0.22, ease: CONTENT_EASE },
        }}
      >
        <AnimatePresence initial={false}>
          {surface === "open" ? (
            <motion.div
              key="panel"
              className="absolute inset-0 overflow-hidden px-2.5 pt-2.5"
              style={{
                borderBottomLeftRadius: "inherit",
                borderBottomRightRadius: "inherit",
              }}
              initial={
                reducedMotion
                  ? { opacity: 0 }
                  : { opacity: 0, filter: "blur(6px)", y: 6, scale: 0.985 }
              }
              animate={
                reducedMotion ? { opacity: 1 } : { opacity: 1, filter: "blur(0px)", y: 0, scale: 1 }
              }
              exit={
                reducedMotion
                  ? { opacity: 0, transition: { duration: 0.15 } }
                  : {
                      opacity: 0,
                      filter: "blur(6px)",
                      scale: 0.985,
                      transition: { duration: 0.12 },
                    }
              }
              transition={{
                delay: reducedMotion ? 0 : 0.13,
                duration: reducedMotion ? 0.15 : 0.24,
                ease: CONTENT_EASE,
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
                      ease: CONTENT_EASE,
                    }}
                  >
                    {content[tab]}
                  </motion.div>
                </AnimatePresence>
              </div>
            </motion.div>
          ) : surface === "compact" && topActivity ? (
            <motion.div
              key="compact"
              className="absolute inset-x-0 top-0 flex h-[30px] justify-center"
              initial={
                reducedMotion ? { opacity: 0 } : { opacity: 0, filter: "blur(6px)", scale: 1.03 }
              }
              animate={
                reducedMotion ? { opacity: 1 } : { opacity: 1, filter: "blur(0px)", scale: 1 }
              }
              exit={
                reducedMotion
                  ? { opacity: 0, transition: { duration: 0.15 } }
                  : {
                      opacity: 0,
                      filter: "blur(6px)",
                      scale: 1.03,
                      transition: { duration: 0.16 },
                    }
              }
              transition={{
                delay: reducedMotion ? 0 : 0.08,
                duration: reducedMotion ? 0.15 : 0.22,
                ease: CONTENT_EASE,
              }}
            >
              <CompactBar
                activity={topActivity}
                onWidthChange={handleBarWidth}
                onClick={openPinnedFromBar}
              />
            </motion.div>
          ) : null}
        </AnimatePresence>
      </motion.div>
    </div>
  );
}
