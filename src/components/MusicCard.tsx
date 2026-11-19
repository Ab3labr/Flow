import { AnimatePresence, motion } from "motion/react";
import {
  AudioLines,
  Captions,
  Ellipsis,
  Pause,
  Play,
  Rewind,
  SkipForward,
  Speaker,
} from "lucide-react";
import { useEffect, useState } from "react";
import { isMockMode } from "../services/env";
import type { Track } from "../services";
import { Equalizer } from "./CollapsedPill";
import { PressButton } from "./TopBar";

function formatTime(seconds: number) {
  const value = Math.max(0, Math.round(seconds));
  return `${Math.floor(value / 60)}:${String(value % 60).padStart(2, "0")}`;
}

export function MusicCard({
  track,
  playing,
  progress,
  volume,
  onPlayingChange,
  onSkip,
  onScrub,
  onVolumeChange,
}: {
  track: Track;
  playing: boolean;
  progress: number;
  volume: number;
  onPlayingChange: (playing: boolean) => void;
  onSkip: (direction: number) => void;
  onScrub: (progress: number) => void;
  onVolumeChange: (volume: number) => void;
}) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [glowColor, setGlowColor] = useState<string | null>(null);
  const duration =
    track.durationMs !== undefined
      ? track.durationMs === null
        ? null
        : track.durationMs / 1000
      : track.duration;
  const totalSeconds = duration ?? 0;
  const hasDuration = totalSeconds > 0;
  const percent = hasDuration ? Math.min(100, (progress / totalSeconds) * 100) : 0;

  // Glow colour: 8x8 downscale of the album art, averaged (design note says
  // "real: sample colors from album art").
  useEffect(() => {
    if (!track.artDataUrl) {
      setGlowColor(null);
      return;
    }
    let cancelled = false;
    const image = new Image();
    image.onload = () => {
      if (cancelled) return;
      const canvas = document.createElement("canvas");
      canvas.width = 8;
      canvas.height = 8;
      const context = canvas.getContext("2d", { willReadFrequently: true });
      if (!context) return;
      context.drawImage(image, 0, 0, 8, 8);
      const { data } = context.getImageData(0, 0, 8, 8);
      let red = 0;
      let green = 0;
      let blue = 0;
      let count = 0;
      for (let index = 0; index < data.length; index += 4) {
        if (data[index + 3] === 0) continue;
        red += data[index];
        green += data[index + 1];
        blue += data[index + 2];
        count += 1;
      }
      if (!count) return;
      setGlowColor(
        `rgb(${Math.round(red / count)}, ${Math.round(green / count)}, ${Math.round(blue / count)})`,
      );
    };
    image.onerror = () => {
      if (!cancelled) setGlowColor(null);
    };
    image.src = track.artDataUrl;
    return () => {
      cancelled = true;
    };
  }, [track.artDataUrl]);

  return (
    <motion.div
      layout
      transition={{ type: "spring", stiffness: 380, damping: 30, mass: 0.9 }}
      className="relative h-full min-w-0 flex-1 overflow-hidden rounded-[14px] border border-[#1f1f1f] bg-[#121212] p-2.5"
    >
      <AnimatePresence mode="popLayout">
        <motion.div
          key={`glow-${track.id}`}
          className="pointer-events-none absolute -left-5 -top-7 h-28 w-40 opacity-[0.12] blur-[60px]"
          style={{
            background:
              glowColor ?? `linear-gradient(135deg, ${track.colors.join(", ")})`,
          }}
          initial={{ opacity: 0 }}
          animate={{ opacity: 0.12 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.5 }}
        />
      </AnimatePresence>

      <div className="relative flex items-start">
        <AnimatePresence mode="wait">
          <motion.div
            key={track.id}
            className="h-12 w-12 shrink-0 overflow-hidden rounded-[11px] shadow-[0_8px_24px_rgba(0,0,0,.28)]"
            style={{ background: `linear-gradient(135deg, ${track.colors.join(", ")})` }}
            initial={{ opacity: 0, scale: 0.96 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.96 }}
            transition={{ duration: 0.26 }}
          >
            {track.artDataUrl ? (
              <img
                src={track.artDataUrl}
                alt=""
                draggable={false}
                className="h-full w-full object-cover"
              />
            ) : (
              <div className="flex h-full items-end p-2">
                <AudioLines className="h-4 w-4 text-white/75" strokeWidth={1.5} />
              </div>
            )}
          </motion.div>
        </AnimatePresence>
        <AnimatePresence mode="wait">
          <motion.div
            key={`copy-${track.id}`}
            className="min-w-0 flex-1 pl-2.5 pt-1"
            initial={{ opacity: 0, y: 5 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -5 }}
            transition={{ duration: 0.26 }}
          >
            <p className="truncate text-[13px] font-semibold leading-[18px] text-white">
              {track.title}
            </p>
            <p className="truncate text-[11px] leading-4 text-white/60">{track.artist}</p>
            <p className="mt-0.5 text-[11px] text-white/35">{track.source}</p>
          </motion.div>
        </AnimatePresence>
        <Equalizer playing={playing} />
      </div>

      {hasDuration ? (
        <div className="mt-1.5 flex items-center gap-2">
          <span className="w-7 text-[11px] tabular-nums text-white/50">
            {formatTime(progress)}
          </span>
          <div className="group relative flex h-3 flex-1 items-center">
            {isMockMode && (
              <input
                aria-label="Track progress"
                type="range"
                min="0"
                max={totalSeconds}
                step="0.1"
                value={progress}
                onChange={(event) => onScrub(Number(event.target.value))}
                onClick={(event) => event.stopPropagation()}
                className="media-range absolute inset-0 z-10 w-full cursor-pointer opacity-0"
              />
            )}
            <div className="h-1 w-full overflow-hidden rounded-full bg-white/15 transition-[height] duration-150 group-hover:h-1.5">
              <div className="h-full rounded-full bg-white" style={{ width: `${percent}%` }} />
            </div>
            <div
              className="pointer-events-none absolute h-2.5 w-2.5 -translate-x-1/2 scale-0 rounded-full bg-white opacity-0 transition group-hover:scale-100 group-hover:opacity-100"
              style={{ left: `${percent}%` }}
            />
          </div>
          <span className="w-8 text-right text-[11px] tabular-nums text-white/50">
            -{formatTime(totalSeconds - progress)}
          </span>
        </div>
      ) : (
        // Unknown duration (radio/live): keep the card height, hide the bar.
        <div className="mt-1.5 h-3" />
      )}

      {isMockMode ? (
        <div className="flex h-[22px] items-center justify-between text-white/75">
          <PressButton
            className="grid h-[22px] w-[22px] place-items-center rounded-full hover:bg-white/[0.06]"
            aria-label="Lyrics"
          >
            <Captions className="h-4 w-4" strokeWidth={1.7} />
          </PressButton>
          <div className="ml-auto flex items-center gap-1">
            <PressButton
              className="grid h-[22px] w-[22px] place-items-center rounded-full hover:bg-white/[0.06]"
              aria-label="Previous track"
              onClick={(event) => {
                event.stopPropagation();
                onSkip(-1);
              }}
            >
              <Rewind className="h-[17px] w-[17px] fill-white text-white" strokeWidth={1.2} />
            </PressButton>
            <PressButton
              className="relative grid h-[26px] w-[26px] place-items-center rounded-full text-white hover:bg-white/[0.07]"
              aria-label={playing ? "Pause" : "Play"}
              onClick={(event) => {
                event.stopPropagation();
                onPlayingChange(!playing);
              }}
            >
              <motion.span
                className="absolute"
                animate={{
                  opacity: playing ? 1 : 0,
                  scale: playing ? 1 : 0.55,
                  rotate: playing ? 0 : -20,
                }}
              >
                <Pause className="h-[22px] w-[22px] fill-white" strokeWidth={0} />
              </motion.span>
              <motion.span
                className="absolute"
                animate={{
                  opacity: playing ? 0 : 1,
                  scale: playing ? 0.55 : 1,
                  rotate: playing ? 20 : 0,
                }}
              >
                <Play className="ml-0.5 h-[22px] w-[22px] fill-white" strokeWidth={0} />
              </motion.span>
            </PressButton>
            <PressButton
              className="grid h-[22px] w-[22px] place-items-center rounded-full hover:bg-white/[0.06]"
              aria-label="Next track"
              onClick={(event) => {
                event.stopPropagation();
                onSkip(1);
              }}
            >
              <SkipForward className="h-[17px] w-[17px] fill-white text-white" strokeWidth={1.2} />
            </PressButton>
          </div>
          <div className="relative">
            <PressButton
              className="grid h-[22px] w-[22px] place-items-center rounded-full hover:bg-white/[0.06]"
              aria-label="More media controls"
              onClick={(event) => {
                event.stopPropagation();
                setMenuOpen((open) => !open);
              }}
            >
              <Ellipsis className="h-4 w-4" />
            </PressButton>
            <AnimatePresence>
              {menuOpen && (
                <motion.div
                  initial={{ opacity: 0, y: 5, scale: 0.97 }}
                  animate={{ opacity: 1, y: 0, scale: 1 }}
                  exit={{ opacity: 0, y: 4, scale: 0.97 }}
                  className="absolute bottom-8 right-0 z-20 w-40 rounded-xl border border-white/10 bg-[#202020] p-3 shadow-xl"
                  onClick={(event) => event.stopPropagation()}
                >
                  <div className="mb-3 flex items-center gap-2">
                    <Speaker className="h-3.5 w-3.5 text-white/60" />
                    <input
                      aria-label="Volume"
                      className="volume-range w-full"
                      type="range"
                      min="0"
                      max="100"
                      value={volume}
                      onChange={(event) => onVolumeChange(Number(event.target.value))}
                    />
                  </div>
                  <button className="w-full rounded-md py-1 text-left text-[11px] text-white/70 hover:text-white">
                    Output device
                  </button>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </div>
      ) : (
        // Transport + volume are not wired to SMTC yet (Phase 3B).
        <div className="h-[22px]" />
      )}
    </motion.div>
  );
}
