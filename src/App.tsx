import { BatteryMedium, ChevronUp, Search, Volume2, Wifi } from "lucide-react";
import { useEffect, useState } from "react";
import { Notch } from "./components/Notch";

function WindowsMark() {
  return (
    <div className="grid h-[18px] w-[18px] grid-cols-2 gap-[2px]">
      <span className="bg-[#57a8ef]" />
      <span className="bg-[#57a8ef]" />
      <span className="bg-[#57a8ef]" />
      <span className="bg-[#57a8ef]" />
    </div>
  );
}

function Taskbar() {
  return (
    <div className="absolute inset-x-0 bottom-0 flex h-12 items-center justify-center border-t border-white/10 bg-[#0a1c28]/80 backdrop-blur-xl">
      <div className="flex items-center gap-2">
        <button
          className="grid h-9 w-9 place-items-center rounded-md hover:bg-white/10"
          aria-label="Start"
        >
          <WindowsMark />
        </button>
        <button
          className="flex h-8 w-[132px] items-center gap-2 rounded-full border border-white/10 bg-white/10 px-3 text-[11px] text-white/60"
          aria-label="Search"
        >
          <Search className="h-3.5 w-3.5" />
          Search
        </button>
        <button
          className="grid h-9 w-9 place-items-center rounded-md bg-white/10"
          aria-label="Open application"
        >
          <div className="h-5 w-5 rounded-md bg-gradient-to-br from-[#53b9ef] to-[#2451ae]" />
        </button>
      </div>
      <div className="absolute right-3 flex items-center gap-2 text-white/80">
        <ChevronUp className="h-3.5 w-3.5" />
        <Wifi className="h-3.5 w-3.5" />
        <Volume2 className="h-3.5 w-3.5" />
        <BatteryMedium className="h-4 w-4" />
        <div className="ml-1 text-right text-[10px] leading-[13px] tabular-nums">
          <div>2:42 PM</div>
          <div>11/4/2026</div>
        </div>
      </div>
    </div>
  );
}

export default function App() {
  const [scale, setScale] = useState(() =>
    Math.min(window.innerWidth / 1920, window.innerHeight / 1080),
  );

  useEffect(() => {
    const fitStage = () => setScale(Math.min(window.innerWidth / 1920, window.innerHeight / 1080));
    fitStage();
    window.addEventListener("resize", fitStage);
    return () => window.removeEventListener("resize", fitStage);
  }, []);

  return (
    <main className="relative h-screen w-screen overflow-hidden bg-[#02080d]">
      <div
        className="absolute left-1/2 top-1/2 h-[1080px] w-[1920px] overflow-hidden bg-[#06131b]"
        style={{ transform: `translate(-50%, -50%) scale(${scale})` }}
      >
        <div
          className="absolute inset-0 bg-cover bg-center"
          style={{
            backgroundImage:
              "linear-gradient(to bottom, rgba(1,9,19,.08), rgba(2,15,23,.2)), url('https://images.unsplash.com/photo-1786855435965-2dd5e12fa50b?crop=entropy&cs=tinysrgb&fit=crop&fm=jpg&q=88&w=2400')",
          }}
        />
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_35%,transparent_20%,rgba(0,8,12,.22)_100%)]" />
        <Notch />
        <Taskbar />
      </div>
    </main>
  );
}
