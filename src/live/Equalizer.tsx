import { motion } from "motion/react";

export function Equalizer({ playing, color = "#f09ab7" }: { playing: boolean; color?: string }) {
  return (
    <div className="flex h-3 items-end gap-[2px]" aria-label={playing ? "Playing" : "Paused"}>
      {[0.88, 1.12, 1.28].map((duration, index) => (
        <motion.span
          key={duration}
          className="block w-[2px] rounded-full"
          style={{ backgroundColor: color }}
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
