import { motion } from "motion/react";
import type { Track } from "../services/mockServices";

function Equalizer({ playing }: { playing: boolean }) {
  return (
    <div className="flex h-3 items-end gap-[2px]" aria-label={playing ? "Playing" : "Paused"}>
      {[0.88, 1.12, 1.28].map((duration, index) => (
        <motion.span
          key={duration}
          className="block w-[2px] rounded-full bg-[#f09ab7]"
          animate={{ height: playing ? [3, 10 - index, 5, 11, 3] : 3 }}
          transition={{
            duration,
            repeat: playing ? Infinity : 0,
            ease: "easeInOut",
            delay: index * 0.08,
          }}
        />
      ))}
    </div>
  );
}

export function CollapsedPill({
  track,
  playing,
  recording,
  recordingTime,
}: {
  track: Track;
  playing: boolean;
  recording: boolean;
  recordingTime: number;
}) {
  const displayTime = `${Math.floor(recordingTime / 60)}:${String(recordingTime % 60).padStart(2, "0")}`;

  return (
    <div className="flex h-full w-full items-center justify-between px-2">
      {recording ? (
        <>
          <span className="relative ml-1 flex h-2.5 w-2.5">
            <motion.span
              className="absolute inset-0 rounded-full bg-[#ff453a]"
              animate={{ opacity: [0.55, 0, 0.55], scale: [1, 2, 1] }}
              transition={{ duration: 1.8, repeat: Infinity }}
            />
            <span className="relative h-2.5 w-2.5 rounded-full bg-[#ff453a]" />
          </span>
          <span className="mr-1 text-[11px] font-medium tabular-nums text-white/70">
            {displayTime}
          </span>
        </>
      ) : playing ? (
        <>
          <div
            className="h-5 w-5 rounded-[6px]"
            style={{ background: `linear-gradient(135deg, ${track.colors.join(", ")})` }}
          />
          <Equalizer playing={playing} />
        </>
      ) : null}
    </div>
  );
}

export { Equalizer };
