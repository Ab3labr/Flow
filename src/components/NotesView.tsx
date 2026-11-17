import { Check, List, Plus, Trash2 } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { Note, noteService } from "../services/mockServices";
import { PressButton } from "./TopBar";

function timeAgo(timestamp: number) {
  const minutes = Math.max(0, Math.floor((Date.now() - timestamp) / 60000));
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `${hours}h ago` : `${Math.floor(hours / 24)}d ago`;
}

export function NotesView({
  newNoteRequest,
  onHeightChange,
  onRequestHandled,
}: {
  newNoteRequest: number;
  onHeightChange: (height: number) => void;
  onRequestHandled: () => void;
}) {
  const initial = useRef<Note | null>(null);
  if (!initial.current) {
    initial.current = noteService.list()[0] || {
      id: crypto.randomUUID(),
      title: "",
      body: "",
      updatedAt: Date.now(),
    };
  }
  const [note, setNote] = useState(initial.current);
  const [cardHeight, setCardHeight] = useState(120);
  const [saved, setSaved] = useState(true);
  const [listOpen, setListOpen] = useState(false);
  const [recent, setRecent] = useState(noteService.list());
  const titleRef = useRef<HTMLInputElement>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const saveTimer = useRef<number | undefined>(undefined);

  const refreshHeight = () => {
    window.requestAnimationFrame(() => {
      if (!bodyRef.current) return;
      const contentHeight = Math.max(42, bodyRef.current.scrollHeight);
      const cardHeight = Math.min(288, Math.max(120, contentHeight + 68));
      setCardHeight(cardHeight);
      onHeightChange(cardHeight + 52);
    });
  };

  useEffect(refreshHeight, [note.body]);

  useEffect(() => {
    if (!newNoteRequest) return;
    const created = noteService.create();
    setNote(created);
    setRecent(noteService.list());
    setSaved(true);
    onRequestHandled();
    window.setTimeout(() => titleRef.current?.focus(), 30);
  }, [newNoteRequest, onRequestHandled]);

  useEffect(() => {
    return () => window.clearTimeout(saveTimer.current);
  }, []);

  const update = (changes: Partial<Note>) => {
    const next = { ...note, ...changes };
    setNote(next);
    setSaved(false);
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      const stored = noteService.save(next);
      setNote(stored);
      setRecent(noteService.list());
      setSaved(true);
    }, 600);
  };

  const createNote = () => {
    const created = noteService.create();
    setNote(created);
    setRecent(noteService.list());
    setSaved(true);
    setListOpen(false);
    window.setTimeout(() => titleRef.current?.focus(), 30);
  };

  return (
    <motion.div
      layout
      transition={{ type: "spring", stiffness: 380, damping: 30, mass: 0.9 }}
      className="flex min-h-[120px] max-h-[288px] flex-col overflow-hidden rounded-[14px] bg-[#121212]"
      animate={{ height: cardHeight }}
    >
      <div className="flex min-h-0 flex-1 flex-col px-3 pt-2.5">
        <input
          ref={titleRef}
          value={note.title}
          onChange={(event) => update({ title: event.target.value })}
          placeholder="Start typing…"
          className="h-6 shrink-0 bg-transparent text-[14px] font-semibold text-white placeholder:text-white/30"
        />
        <textarea
          ref={bodyRef}
          value={note.body}
          onChange={(event) => update({ body: event.target.value })}
          onInput={refreshHeight}
          className="notch-scroll min-h-[42px] flex-1 resize-none overflow-y-auto bg-transparent text-[13px] leading-[1.5] text-white/75 caret-white"
        />
      </div>
      <div className="flex h-6 shrink-0 items-center justify-between border-t border-white/[0.05] px-3">
        <AnimatePresence>
          {saved && (
            <motion.span
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-1 text-[11px] text-white/35"
            >
              <Check className="h-3 w-3" /> Saved
            </motion.span>
          )}
        </AnimatePresence>
        <div className="relative ml-auto flex items-center">
          <PressButton
            onClick={createNote}
            className="flex h-6 items-center gap-1 rounded-md px-2 text-[11px] text-white/55 hover:bg-white/[0.06] hover:text-white"
          >
            <Plus className="h-3 w-3" /> New note
          </PressButton>
          <PressButton
            aria-label="Recent notes"
            onClick={() => setListOpen((open) => !open)}
            className="grid h-6 w-6 place-items-center rounded-md text-white/55 hover:bg-white/[0.06] hover:text-white"
          >
            <List className="h-3.5 w-3.5" />
          </PressButton>
          <AnimatePresence>
            {listOpen && (
              <motion.div
                initial={{ opacity: 0, y: 5, scale: 0.97 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: 4, scale: 0.97 }}
                className="absolute bottom-7 right-0 z-30 w-56 rounded-xl border border-white/10 bg-[#202020] p-1.5 shadow-xl"
              >
                {recent.slice(0, 5).map((item) => (
                  <button
                    key={item.id}
                    onClick={() => {
                      setNote(item);
                      setSaved(true);
                      setListOpen(false);
                    }}
                    className="group flex h-10 w-full items-center rounded-lg px-2 text-left hover:bg-white/[0.06]"
                  >
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[11px] text-white/80">
                        {item.title || "Untitled note"}
                      </span>
                      <span className="block text-[11px] text-white/35">
                        {timeAgo(item.updatedAt)}
                      </span>
                    </span>
                    <span
                      role="button"
                      aria-label="Delete note"
                      onClick={(event) => {
                        event.stopPropagation();
                        noteService.remove(item.id);
                        setRecent(noteService.list());
                      }}
                      className="grid h-6 w-6 place-items-center rounded-md text-white/30 opacity-0 hover:bg-white/10 hover:text-white group-hover:opacity-100"
                    >
                      <Trash2 className="h-3 w-3" />
                    </span>
                  </button>
                ))}
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>
    </motion.div>
  );
}
