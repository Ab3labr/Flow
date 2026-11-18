import { Scissors, Square, Video } from "lucide-react";
import { motion } from "motion/react";
import { captureService } from "../services";
import { PressButton } from "./TopBar";

export function CaptureView({
  recording,
  elapsed,
  onRecordingChange,
}: {
  recording: boolean;
  elapsed: number;
  onRecordingChange: (recording: boolean) => void;
}) {
  const [snipLabel, recordLabel] = captureService.getModes();
  const display = `${String(Math.floor(elapsed / 60)).padStart(2, "0")}:${String(elapsed % 60).padStart(2, "0")}`;
  return (
    <div className="flex h-[133px] items-center gap-2.5">
      <PressButton className="flex h-[100px] flex-1 flex-col items-center justify-center gap-2.5 rounded-[14px] border border-[#1f1f1f] bg-[#121212] text-white/75 hover:bg-[#171717] hover:text-white">
        <Scissors className="h-5 w-5" strokeWidth={1.5} />
        <span className="text-[13px] font-semibold">{snipLabel}</span>
      </PressButton>
      <PressButton
        onClick={() => onRecordingChange(!recording)}
        className="relative flex h-[100px] flex-1 flex-col items-center justify-center gap-2.5 overflow-hidden rounded-[14px] border border-[#1f1f1f] bg-[#121212] text-white/75 hover:bg-[#171717] hover:text-white"
      >
        {recording ? (
          <>
            <motion.span
              className="absolute h-14 w-14 rounded-full bg-[#ff453a]/15 blur-xl"
              animate={{ opacity: [0.35, 0.85, 0.35], scale: [0.8, 1.25, 0.8] }}
              transition={{ duration: 1.7, repeat: Infinity }}
            />
            <Square className="relative h-5 w-5 fill-[#ff453a] text-[#ff453a]" />
            <span className="relative text-[13px] font-semibold tabular-nums">{display}</span>
          </>
        ) : (
          <>
            <span className="h-6 w-6 rounded-full border-[5px] border-[#ff453a]" />
            <span className="text-[13px] font-semibold">{recordLabel}</span>
          </>
        )}
      </PressButton>
    </div>
  );
}
