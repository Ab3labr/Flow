import { Note, noteService } from "../services";
import { chooseNoteOnOpen, DEFAULT_NOTES_NEW_AFTER_MINUTES } from "./chooseNoteOnOpen";
import { isNoteTooLarge, safeNote } from "./noteData";

export type NoteSaveStatus = "saved" | "dirty" | "saving" | "error" | "too-large";

export type NoteSessionSnapshot = {
  id: string | null;
  title: string;
  body: string;
  dirty: boolean;
  status: NoteSaveStatus;
  /** Ask the editor to focus the title (used by New note). */
  focusTitle: boolean;
};

export type NoteSessionSettings = {
  notesLastId: string | null;
  notesLastEditedMs: number;
  notesNewAfterMinutes: number;
};

type NoteServiceLike = {
  get?: (id: string) => unknown;
  save?: (id: string | null, title: string, body: string) => unknown;
};

export type NoteSessionOptions = {
  /** Latest settings, read on demand (the session outlives every tab). */
  getSettings: () => NoteSessionSettings;
  /** Persist the session pointers into settings.json. */
  persist: (lastId: string | null, lastEditedMs: number) => void;
  service?: NoteServiceLike;
  now?: () => number;
  autosaveMs?: number;
};

export type NoteSession = {
  getSnapshot: () => NoteSessionSnapshot;
  subscribe: (listener: () => void) => () => void;
  update: (patch: { title?: string; body?: string }) => void;
  markTooLarge: () => void;
  setCaret: (value: number) => void;
  setScroll: (value: number) => void;
  getCaret: () => number;
  getScroll: () => number;
  flush: () => Promise<void>;
  openForTab: () => Promise<void>;
  startNewNote: () => Promise<void>;
  openNote: (id: string) => Promise<boolean>;
  noteDeleted: (id: string) => void;
  consumeFocusTitle: () => void;
};

const DEFAULT_AUTOSAVE_MS = 600;

function expired(lastEditedMs: number, newAfterMinutes: number, nowMs: number): boolean {
  if (!Number.isFinite(newAfterMinutes) || newAfterMinutes <= 0) return false;
  if (!Number.isFinite(lastEditedMs) || lastEditedMs <= 0) return true;
  return nowMs - lastEditedMs > newAfterMinutes * 60_000;
}

/**
 * Editing state that lives above NotesView, so closing the panel or changing
 * tabs never drops text, the save queue, the caret or the scroll position.
 * React-free so it can be unit-tested and reused by the Notch shell.
 */
export function createNoteSession(options: NoteSessionOptions): NoteSession {
  const service = (options.service ?? noteService) as NoteServiceLike;
  const now = options.now ?? Date.now;
  const autosaveMs = options.autosaveMs ?? DEFAULT_AUTOSAVE_MS;
  const listeners = new Set<() => void>();

  let snapshot: NoteSessionSnapshot = {
    id: null,
    title: "",
    body: "",
    dirty: false,
    status: "saved",
    focusTitle: false,
  };
  let caret = 0;
  let scroll = 0;
  let lastEditedMs = 0;
  let opened = false;
  let disposed = false;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let saveChain: Promise<void> = Promise.resolve();
  let opening: Promise<void> | null = null;

  const emit = (patch: Partial<NoteSessionSnapshot>) => {
    snapshot = { ...snapshot, ...patch };
    for (const listener of listeners) listener();
  };

  const clearTimer = () => {
    if (saveTimer === undefined) return;
    clearTimeout(saveTimer);
    saveTimer = undefined;
  };

  const oversized = () => isNoteTooLarge(snapshot.title, snapshot.body);

  const persist = (id: string | null) => {
    try {
      options.persist(id, lastEditedMs);
    } catch {
      // A settings write must never take down the note editor.
    }
  };

  const readNote = async (id: string): Promise<Note | null> => {
    if (typeof service.get !== "function") return null;
    try {
      const raw = await Promise.resolve(service.get(id));
      return raw ? safeNote(raw, id) : null;
    } catch {
      return null;
    }
  };

  const applyNote = (note: Note) => {
    lastEditedMs = note.updatedAt;
    caret = note.body.length;
    scroll = 0;
    emit({
      id: note.id,
      title: note.title,
      body: note.body,
      dirty: false,
      status: "saved",
      focusTitle: false,
    });
  };

  const blank = (focusTitle: boolean) => {
    lastEditedMs = now();
    caret = 0;
    scroll = 0;
    emit({
      id: null,
      title: "",
      body: "",
      dirty: false,
      status: "saved",
      focusTitle,
    });
    persist(null);
  };

  const schedule = () => {
    clearTimer();
    if (snapshot.status === "too-large") return;
    saveTimer = setTimeout(() => {
      saveTimer = undefined;
      void flush();
    }, autosaveMs);
  };

  const saveOnce = async () => {
    if (disposed || !snapshot.dirty || snapshot.status === "too-large") return;

    const target = { id: snapshot.id, title: snapshot.title, body: snapshot.body };
    if (!target.title.trim() && !target.body.trim()) {
      emit({ dirty: false, status: "saved" });
      persist(target.id);
      return;
    }
    if (typeof service.save !== "function") {
      emit({ status: "error" });
      return;
    }

    emit({ status: "saving" });
    try {
      const stored = await Promise.resolve(service.save(target.id, target.title, target.body));
      const storedId =
        stored && typeof (stored as { id?: unknown }).id === "string"
          ? (stored as { id: string }).id
          : target.id;
      const unchanged = snapshot.title === target.title && snapshot.body === target.body;
      emit({
        id: storedId,
        dirty: !unchanged,
        status: unchanged ? "saved" : "dirty",
      });
      persist(storedId);
      if (!unchanged) schedule();
    } catch {
      emit({ status: "error" });
    }
  };

  const flush = (): Promise<void> => {
    clearTimer();
    const run = async () => {
      for (let i = 0; i < 4; i += 1) {
        if (!snapshot.dirty || snapshot.status === "too-large") return;
        await saveOnce();
        if (snapshot.status === "error") return;
      }
    };
    saveChain = saveChain.then(run, run);
    return saveChain;
  };

  const update = (patch: { title?: string; body?: string }) => {
    const title = patch.title ?? snapshot.title;
    const body = patch.body ?? snapshot.body;
    if (title === snapshot.title && body === snapshot.body) return;

    lastEditedMs = now();
    const tooLarge = isNoteTooLarge(title, body);
    emit({
      title,
      body,
      dirty: true,
      status: tooLarge ? "too-large" : "dirty",
    });
    if (tooLarge) {
      clearTimer();
      return;
    }
    schedule();
  };

  const markTooLarge = () => {
    clearTimer();
    lastEditedMs = now();
    emit({ status: "too-large" });
  };

  const doOpenForTab = async () => {
    const settings = options.getSettings();
    const newAfter =
      typeof settings.notesNewAfterMinutes === "number"
        ? settings.notesNewAfterMinutes
        : DEFAULT_NOTES_NEW_AFTER_MINUTES;

    if (opened) {
      // A prevented paste leaves the text unchanged; clear the warning so the
      // normal open rule applies again.
      if (snapshot.status === "too-large" && !oversized()) {
        emit({ status: snapshot.dirty ? "dirty" : "saved" });
      }
      if (snapshot.dirty) await flush();
      // Never replace text that could not be saved.
      if (snapshot.status === "error" || snapshot.status === "too-large") return;
      if (expired(lastEditedMs, newAfter, now())) blank(false);
      return;
    }

    opened = true;
    const lastId =
      typeof settings.notesLastId === "string" && settings.notesLastId
        ? settings.notesLastId
        : null;
    const lastEditedSetting =
      typeof settings.notesLastEditedMs === "number" &&
      Number.isFinite(settings.notesLastEditedMs)
        ? settings.notesLastEditedMs
        : 0;
    const existing = lastId ? await readNote(lastId) : null;
    // The file mtime is a useful fallback if settings were lost or old.
    const lastEdited = Math.max(lastEditedSetting, existing?.updatedAt ?? 0);
    const chosen = chooseNoteOnOpen(
      now(),
      lastEdited,
      lastId,
      Boolean(existing),
      newAfter,
      false,
    );
    if (chosen && existing) applyNote(existing);
    else blank(false);
  };

  const openForTab = (): Promise<void> => {
    if (!opening) {
      opening = doOpenForTab().finally(() => {
        opening = null;
      });
    }
    return opening;
  };

  const startNewNote = async (): Promise<void> => {
    if (snapshot.dirty) await flush();
    if (snapshot.status === "error") return;
    // A real oversized note stays put; a prevented paste may be cleared.
    if (snapshot.status === "too-large" && oversized()) return;
    opened = true;
    blank(true);
  };

  const openNote = async (id: string): Promise<boolean> => {
    if (snapshot.id === id && !snapshot.dirty) return true;
    if (snapshot.dirty) await flush();
    if (snapshot.status === "error") return false;
    if (snapshot.status === "too-large" && oversized()) return false;

    const note = await readNote(id);
    if (!note) return false;
    opened = true;
    applyNote(note);
    persist(note.id);
    return true;
  };

  const noteDeleted = (id: string) => {
    if (snapshot.id !== id) return;
    if (!snapshot.title.trim() && !snapshot.body.trim()) {
      blank(false);
      return;
    }
    // Keep the text as a fresh draft so deleting the file cannot lose it.
    lastEditedMs = now();
    emit({ id: null, dirty: true, status: "dirty" });
    schedule();
  };

  return {
    getSnapshot: () => snapshot,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    update,
    markTooLarge,
    setCaret: (value) => {
      caret = Number.isFinite(value) ? value : 0;
    },
    setScroll: (value) => {
      scroll = Number.isFinite(value) ? value : 0;
    },
    getCaret: () => caret,
    getScroll: () => scroll,
    flush,
    openForTab,
    startNewNote,
    openNote,
    noteDeleted,
    consumeFocusTitle: () => {
      if (snapshot.focusTitle) emit({ focusTitle: false });
    },
  };
}
