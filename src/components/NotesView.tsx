import { Check, List, Plus, Trash2 } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import {
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ClipboardEvent,
} from "react";
import { Note, noteService } from "../services";
import { isNoteTooLarge, loadNoteList, timeAgo } from "../notes/noteData";
import type { NoteSession } from "../notes/noteSession";
import { PressButton } from "./TopBar";

const MIN_BODY_HEIGHT = 42;
const MIN_CARD_HEIGHT = 120;
const NOTE_CARD_CHROME = 68;
const PANEL_CHROME = 52;
const MAX_PANEL_HEIGHT = 340;
// The outer panel includes TopBar, its margin and panel padding. Keep the
// card below the approved 340 px panel maximum so the body can scroll.
const MAX_CARD_HEIGHT = MAX_PANEL_HEIGHT - PANEL_CHROME;

/** Card height for a natural textarea height, bounded by the panel viewport. */
export function noteCardHeightForContent(contentHeight: number): number {
  const bodyHeight =
    Number.isFinite(contentHeight) ? Math.max(MIN_BODY_HEIGHT, contentHeight) : MIN_BODY_HEIGHT;
  return Math.min(MAX_CARD_HEIGHT, Math.max(MIN_CARD_HEIGHT, bodyHeight + NOTE_CARD_CHROME));
}

type ServiceExtras = {
  subscribe?: (listener: (event: string) => void) => unknown;
  remove?: (id: string) => unknown;
  delete?: (id: string) => unknown;
  openFolder?: () => unknown;
  reveal?: (id: string) => unknown;
};

export function NotesView({
  session,
  onHeightChange,
  onFlushReady,
}: {
  session: NoteSession;
  onHeightChange: (height: number) => void;
  onFlushReady?: (flush: () => Promise<void>) => void;
}) {
  const snapshot = useSyncExternalStore(
    session.subscribe,
    session.getSnapshot,
    session.getSnapshot,
  );
  const [cardHeight, setCardHeight] = useState(120);
  const [listOpen, setListOpen] = useState(false);
  const [recent, setRecent] = useState<Note[]>([]);
  const titleRef = useRef<HTMLInputElement>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const cardHeightRef = useRef(cardHeight);

  const saving = snapshot.status === "saving";
  const saved = snapshot.status === "saved" && !snapshot.dirty;
  const saveError = snapshot.status === "error";
  const tooLarge = snapshot.status === "too-large";

  const applyHeight = (next: number) => {
    if (next === cardHeightRef.current) return;
    cardHeightRef.current = next;
    setCardHeight(next);
    onHeightChange(next + 52);
  };

  // Measure the textarea's natural content height, not its flexed viewport
  // height. Measuring scrollHeight while it fills the card feeds the current
  // card height back into itself and makes the panel grow on every keystroke.
  // A large paste is still one measurement, so it cannot lock the panel in a
  // layout loop.
  useEffect(() => {
    const element = bodyRef.current;
    if (!element) return;
    const previousHeight = element.style.height;
    const previousGrow = element.style.flexGrow;
    // The textarea is a flex-grown item, so `height: 0` alone still leaves the
    // flex-assigned box in place and scrollHeight reports the *viewport*, not
    // the content. Collapsing the grow factor too gives the natural height.
    element.style.flexGrow = "0";
    element.style.height = "0px";
    const contentHeight = element.scrollHeight;
    element.style.height = previousHeight;
    element.style.flexGrow = previousGrow;
    applyHeight(noteCardHeightForContent(contentHeight));
    // `applyHeight` only depends on the ref and the callback prop.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [snapshot.body, onHeightChange]);

  // Restore the scroll position and put the caret at the end.
  useEffect(() => {
    const element = bodyRef.current;
    if (!element) return;
    element.scrollTop = session.getScroll();
    const end = element.value.length;
    if (end > 0) element.focus();
    element.setSelectionRange(end, end);
  }, [session, snapshot.id]);

  // New note requests focus the title; ordinary reopens leave the view alone.
  useEffect(() => {
    if (!snapshot.focusTitle) return;
    session.consumeFocusTitle();
    const timer = window.setTimeout(() => titleRef.current?.focus(), 30);
    return () => window.clearTimeout(timer);
  }, [session, snapshot.focusTitle]);

  // Refresh the recent list after a save, an open, or a new draft.
  useEffect(() => {
    if (snapshot.status !== "saved") return;
    let mounted = true;
    void loadNoteList().then((rows) => {
      if (mounted) setRecent(rows);
    });
    return () => {
      mounted = false;
    };
  }, [snapshot.status, snapshot.id]);

  // Subscribe only when the active service actually exposes the event.
  useEffect(() => {
    const subscribe = (noteService as ServiceExtras).subscribe;
    if (typeof subscribe !== "function") return;
    let unsubscribe: unknown;
    try {
      unsubscribe = subscribe.call(noteService, (event) => {
        if (event !== "notes-changed") return;
        void loadNoteList().then((rows) => setRecent(rows));
      });
    } catch {
      return;
    }
    return () => {
      if (typeof unsubscribe !== "function") return;
      try {
        (unsubscribe as () => void)();
      } catch {
        // A failed unsubscribe must not take down the Notes tab.
      }
    };
  }, []);

  // Flush when the window hides or the app is about to unload.
  useEffect(() => {
    const flush = () => {
      void session.flush();
    };
    const handleVisibilityChange = () => {
      if (document.hidden) flush();
    };
    document.addEventListener("visibilitychange", handleVisibilityChange);
    window.addEventListener("beforeunload", flush);
    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      window.removeEventListener("beforeunload", flush);
    };
  }, [session]);

  // Let the panel flush a pending draft before it closes or switches tabs.
  useEffect(() => {
    if (!onFlushReady) return;
    onFlushReady(() => session.flush());
    return () => onFlushReady(() => Promise.resolve());
  }, [onFlushReady, session]);

  // Remember the caret/scroll when the tab unmounts.
  useEffect(
    () => () => {
      const element = bodyRef.current;
      if (!element) return;
      session.setCaret(element.selectionStart ?? element.value.length);
      session.setScroll(element.scrollTop);
    },
    [session],
  );

  const revealCurrent = () => {
    if (!snapshot.id) return;
    const reveal = (noteService as ServiceExtras).reveal;
    if (typeof reveal !== "function") return;
    try {
      void Promise.resolve(reveal.call(noteService, snapshot.id)).catch(() => undefined);
    } catch {
      // The footer must stay usable if Explorer cannot be launched.
    }
  };

  const openNotesFolder = () => {
    const open = (noteService as ServiceExtras).openFolder;
    if (typeof open !== "function") return;
    try {
      void Promise.resolve(open.call(noteService)).catch(() => undefined);
    } catch {
      // Ignore shell failures; the editor keeps working.
    }
  };

  const openRecent = async (item: Note) => {
    const opened = await session.openNote(item.id);
    if (opened) {
      setListOpen(false);
      return;
    }
    const rows = await loadNoteList();
    setRecent(rows);
  };

  const deleteRecent = async (item: Note) => {
    const service = noteService as ServiceExtras;
    const remove = service.remove ?? service.delete;
    if (typeof remove === "function") {
      try {
        await Promise.resolve(remove.call(service, item.id)).catch(() => undefined);
      } catch {
        // localStorage or the service can throw synchronously; keep the view up.
      }
    }
    session.noteDeleted(item.id);
    const rows = await loadNoteList();
    setRecent(rows);
  };

  const bodyPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const pasted = event.clipboardData.getData("text");
    const element = event.currentTarget;
    const start = element.selectionStart ?? element.value.length;
    const end = element.selectionEnd ?? start;
    const next = element.value.slice(0, start) + pasted + element.value.slice(end);
    if (isNoteTooLarge(snapshot.title, next)) {
      event.preventDefault();
      session.markTooLarge();
    }
  };

  const titlePaste = (event: ClipboardEvent<HTMLInputElement>) => {
    const pasted = event.clipboardData.getData("text");
    const element = event.currentTarget;
    const start = element.selectionStart ?? element.value.length;
    const end = element.selectionEnd ?? start;
    const next = element.value.slice(0, start) + pasted + element.value.slice(end);
    if (isNoteTooLarge(next, snapshot.body)) {
      event.preventDefault();
      session.markTooLarge();
    }
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
          value={snapshot.title}
          onChange={(event) => session.update({ title: event.target.value })}
          onPaste={titlePaste}
          onKeyDown={(event) => {
            // The title remains a one-line name. Enter continues writing in
            // the multiline note body, where repeated Enters make blank lines.
            if (event.key !== "Enter") return;
            event.preventDefault();
            bodyRef.current?.focus();
          }}
          onSelect={(event) => session.setCaret(event.currentTarget.selectionStart ?? 0)}
          placeholder="Start typing…"
          className="h-6 shrink-0 bg-transparent text-[14px] font-semibold text-white placeholder:text-white/30"
        />
        <textarea
          ref={bodyRef}
          value={snapshot.body}
          onChange={(event) => session.update({ body: event.target.value })}
          onPaste={bodyPaste}
          onSelect={(event) => session.setCaret(event.currentTarget.selectionStart ?? 0)}
          onScroll={(event) => session.setScroll(event.currentTarget.scrollTop)}
          className="notch-scroll min-h-[42px] flex-1 resize-none overflow-y-auto bg-transparent text-[13px] leading-[1.5] text-white/75 caret-white"
        />
      </div>
      <div className="flex h-6 shrink-0 items-center justify-between border-t border-white/[0.05] px-3">
        <AnimatePresence>
          {saved && (
            <motion.button
              type="button"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              title={snapshot.id ?? "Unsaved note"}
              onClick={revealCurrent}
              disabled={!snapshot.id}
              className="flex items-center gap-1 rounded px-1 text-[11px] text-white/35 hover:bg-white/[0.06] hover:text-white disabled:cursor-default disabled:hover:bg-transparent disabled:hover:text-white/35"
            >
              <Check className="h-3 w-3" /> Saved
            </motion.button>
          )}
          {saving && (
            <motion.span
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-1 text-[11px] text-white/35"
            >
              <Check className="h-3 w-3" /> Saving…
            </motion.span>
          )}
          {saveError && (
            <motion.span
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-1 text-[11px] text-[#ff453a]"
            >
              <Check className="h-3 w-3" /> Couldn't save
            </motion.span>
          )}
          {tooLarge && (
            <motion.span
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-1 text-[11px] text-[#ff453a]"
            >
              <Check className="h-3 w-3" /> Too large
            </motion.span>
          )}
        </AnimatePresence>
        <div className="relative ml-auto flex items-center">
          <PressButton
            onClick={() => {
              void session.startNewNote();
            }}
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
                    onClick={() => void openRecent(item)}
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
                        void deleteRecent(item);
                      }}
                      className="grid h-6 w-6 place-items-center rounded-md text-white/30 opacity-0 hover:bg-white/10 hover:text-white group-hover:opacity-100"
                    >
                      <Trash2 className="h-3 w-3" />
                    </span>
                  </button>
                ))}
                <button
                  type="button"
                  onClick={openNotesFolder}
                  className="mt-1 flex h-7 w-full items-center rounded-lg border-t border-white/[0.06] px-2 text-[11px] text-white/45 hover:bg-white/[0.06] hover:text-white"
                >
                  Open notes folder
                </button>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>
    </motion.div>
  );
}
