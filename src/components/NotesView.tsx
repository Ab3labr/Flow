import { Check, List, Plus, Trash2 } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { Note, noteService } from "../services";
import { PressButton } from "./TopBar";

/**
 * The mock service is synchronous and uses `updatedAt`; the Tauri service is
 * async and uses `modifiedMs`. Normalise both at the boundary so the view
 * never renders a Promise or a partially-shaped note.
 */
type NoteRow = {
  id?: unknown;
  title?: unknown;
  body?: unknown;
  updatedAt?: unknown;
  modifiedMs?: unknown;
};

function makeId(): string {
  const c = globalThis.crypto as Crypto | undefined;
  if (c && typeof c.randomUUID === "function") return c.randomUUID();
  return `note-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function safeTimestamp(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : Date.now();
}

function safeNote(value: unknown): Note {
  const row = (value && typeof value === "object" ? value : {}) as NoteRow;
  return {
    id: typeof row.id === "string" && row.id ? row.id : makeId(),
    title: typeof row.title === "string" ? row.title : "",
    body: typeof row.body === "string" ? row.body : "",
    updatedAt: Number.isFinite(row.updatedAt)
      ? (row.updatedAt as number)
      : Number.isFinite(row.modifiedMs)
        ? (row.modifiedMs as number)
        : Date.now(),
  };
}

function safeNoteList(value: unknown): Note[] {
  return (Array.isArray(value) ? value : []).map(safeNote);
}

/** Both services return different shapes; always resolve to `Note[]`. */
function loadNotes(): Promise<Note[]> {
  try {
    return Promise.resolve(noteService.list()).then(safeNoteList, () => []);
  } catch {
    return Promise.resolve([]);
  }
}

/** Create a draft without assuming the mock-only `create` method exists. */
function createDraft(): Note {
  const create = (noteService as { create?: () => unknown }).create;
  if (typeof create === "function") {
    try {
      return safeNote(create.call(noteService));
    } catch {
      // Fall through to a local-only draft.
    }
  }
  return safeNote({ id: null, title: "", body: "", updatedAt: Date.now() });
}

function timeAgo(timestamp: unknown) {
  const minutes = Math.max(0, Math.floor((Date.now() - safeTimestamp(timestamp)) / 60000));
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `${hours}h ago` : `${Math.floor(hours / 24)}d ago`;
}

export function NotesView({
  newNoteRequest,
  onHeightChange,
  onRequestHandled,
  onFlushReady,
}: {
  newNoteRequest: number;
  onHeightChange: (height: number) => void;
  onRequestHandled: () => void;
  onFlushReady?: (flush: () => Promise<void>) => void;
}) {
  const [note, setNote] = useState<Note>(() => safeNote(null));
  const [cardHeight, setCardHeight] = useState(120);
  const [saved, setSaved] = useState(true);
  const [listOpen, setListOpen] = useState(false);
  const [recent, setRecent] = useState<Note[]>([]);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const titleRef = useRef<HTMLInputElement>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const saveTimer = useRef<number | undefined>(undefined);
  const saveQueue = useRef<{ title: string; body: string; id: string | null } | null>(null);
  const mounted = useRef(true);
  const initialised = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      window.clearTimeout(saveTimer.current);
    };
  }, []);

  const refreshHeight = () => {
    if (!bodyRef.current) return;
    const contentHeight = Math.max(42, bodyRef.current.scrollHeight);
    const newCardHeight = Math.min(340, Math.max(172, contentHeight + 68));
    if (newCardHeight !== cardHeight) {
      setCardHeight(newCardHeight);
      onHeightChange(newCardHeight + 52);
    }
  };

  // Initial list load. A failed or non-array response simply yields no notes.
  useEffect(() => {
    void loadNotes().then((rows) => {
      if (!mounted.current) return;
      setRecent(rows);
      if (!initialised.current) {
        initialised.current = true;
        if (rows.length > 0) setNote(rows[0]);
      }
    });
  }, []);

  // Subscribe only when the active service actually exposes the event.
  useEffect(() => {
    const subscribe = (
      noteService as { subscribe?: (listener: (event: string) => void) => unknown }
    ).subscribe;
    if (typeof subscribe !== "function") return;
    let unsubscribe: unknown;
    try {
      unsubscribe = subscribe.call(noteService, (event) => {
        if (event === "notes-changed") {
          void loadNotes().then((rows) => {
            if (mounted.current) setRecent(rows);
          });
        }
      });
    } catch {
      return;
    }
    return () => {
      if (typeof unsubscribe === "function") (unsubscribe as () => void)();
    };
  }, []);

  // Queue edits that arrive while a save is in flight.
  useEffect(() => {
    if (saving) {
      saveQueue.current = { title: note.title, body: note.body, id: note.id ?? null };
    }
  }, [note, saving]);

  const doSave = async () => {
    if (!note.title.trim() && !note.body.trim()) {
      setSaved(true);
      setSaveError(null);
      return;
    }

    setSaving(true);
    setSaveError(null);

    try {
      const stored = await noteService.save(note.id ?? null, note.title, note.body);
      const storedId = (stored as { id?: unknown } | null)?.id;
      if (mounted.current) {
        // Never replace the editor state with the partial save result.
        setNote((current) => ({ ...current, id: typeof storedId === "string" ? storedId : current.id }));
        setSaved(true);
        setSaveError(null);
        setSaving(false);
      }
      // A stale queued snapshot would disable the debounce on later edits.
      saveQueue.current = null;
      void loadNotes().then((rows) => {
        if (mounted.current) setRecent(rows);
      });
    } catch {
      if (mounted.current) {
        console.error("Save failed");
        setSaveError("Couldn't save");
        setSaving(false);
      }
    }
  };

  const update = (changes: Partial<Note>) => {
    const next = { ...note, ...changes };
    setNote(next);
    setSaved(false);
    window.clearTimeout(saveTimer.current);

    if (saveQueue.current) {
      saveQueue.current = { title: next.title, body: next.body, id: next.id ?? null };
    } else {
      saveTimer.current = window.setTimeout(() => {
        doSave();
      }, 600);
    }
  };

  const handleSave = async () => {
    window.clearTimeout(saveTimer.current);
    if (!saved || note.title.trim() || note.body.trim()) {
      await doSave();
    }
  };

  // Let the panel flush a pending draft before it closes or switches tabs.
  // The ref keeps the registration stable while `handleSave` sees fresh state.
  const flushRef = useRef<() => Promise<void>>(() => Promise.resolve());
  flushRef.current = handleSave;
  useEffect(() => {
    if (!onFlushReady) return;
    onFlushReady(() => flushRef.current());
    return () => onFlushReady(() => Promise.resolve());
  }, [onFlushReady]);

  // Flush on tab switch, panel close or window blur.
  useEffect(() => {
    const flush = () => {
      window.clearTimeout(saveTimer.current);
      if (!saved || note.title.trim() || note.body.trim()) {
        doSave();
      }
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
  }, [saved, note]);

  // One draft per request: flush the current note first, then create.
  useEffect(() => {
    if (!newNoteRequest) return;
    window.clearTimeout(saveTimer.current);

    const openDraft = () => {
      if (!mounted.current) return;
      setNote(createDraft());
      setSaved(true);
      onRequestHandled();
      void loadNotes().then((rows) => {
        if (mounted.current) setRecent(rows);
      });
      window.setTimeout(() => titleRef.current?.focus(), 30);
    };

    if (!saved || note.title.trim() || note.body.trim()) {
      void doSave().then(openDraft);
    } else {
      openDraft();
    }
    // `saved`/`note` are intentionally excluded: this must fire once per request.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [newNoteRequest]);

  const openRecent = async (item: Note) => {
    window.clearTimeout(saveTimer.current);
    if (!saved || note.title.trim() || note.body.trim()) {
      await doSave();
    }
    void loadNotes().then((rows) => {
      if (mounted.current) setRecent(rows);
    });
    setNote(safeNote(item));
    setListOpen(false);
    setSaved(true);
  };

  const deleteRecent = (item: Note) => {
    const service = noteService as {
      remove?: (id: string) => unknown;
      delete?: (id: string) => unknown;
    };
    const remove = service.remove ?? service.delete;
    if (typeof remove === "function") {
      void Promise.resolve(remove.call(service, item.id)).catch(() => undefined);
    }
    void loadNotes().then((rows) => {
      if (!mounted.current) return;
      setRecent(rows);
      if (item.id === note.id) setNote(createDraft());
    });
  };

  return (
    <motion.div
      layout
      transition={{ type: "spring", stiffness: 380, damping: 30, mass: 0.9 }}
      className="flex min-h-[120px] max-h-[340px] flex-col overflow-hidden rounded-[14px] bg-[#121212]"
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
          {saved && !saving && (
            <motion.span
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-1 text-[11px] text-white/35"
            >
              <Check className="h-3 w-3" /> Saved
            </motion.span>
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
              className="flex items-center gap-1 text-[11px] text-white/35"
            >
              <Check className="h-3 w-3" /> {saveError}
            </motion.span>
          )}
        </AnimatePresence>
        <div className="relative ml-auto flex items-center">
          <PressButton
            onClick={handleSave}
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
                    onClick={() => openRecent(item)}
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
                        deleteRecent(item);
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
