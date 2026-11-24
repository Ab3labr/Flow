import { Note, noteService } from "../services";

/** Hard ceiling for a note body; over this it is not saved. */
export const MAX_NOTE_BYTES = 2 * 1024 * 1024;

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

export function safeTimestamp(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : Date.now();
}

/** Normalise a service row into the front-end `Note` shape. */
export function safeNote(value: unknown, fallbackId?: string): Note {
  const row = (value && typeof value === "object" ? value : {}) as NoteRow;
  return {
    id:
      typeof row.id === "string" && row.id
        ? row.id
        : typeof fallbackId === "string" && fallbackId
          ? fallbackId
          : makeId(),
    title: typeof row.title === "string" ? row.title : "",
    body: typeof row.body === "string" ? row.body : "",
    updatedAt: Number.isFinite(row.updatedAt)
      ? (row.updatedAt as number)
      : Number.isFinite(row.modifiedMs)
        ? (row.modifiedMs as number)
        : Date.now(),
  };
}

export function safeNoteList(value: unknown): Note[] {
  return (Array.isArray(value) ? value : []).map((item) => safeNote(item));
}

/** Both services return different shapes; always resolve to `Note[]`. */
export function loadNoteList(): Promise<Note[]> {
  try {
    return Promise.resolve(noteService.list()).then(safeNoteList, () => []);
  } catch {
    return Promise.resolve([]);
  }
}

const encoder =
  typeof TextEncoder !== "undefined" ? new TextEncoder() : null;

/** UTF-8 size of the note, matching what the file store will write. */
export function noteByteLength(title: string, body: string): number {
  const text = title ? `# ${title}\n\n${body}` : body;
  return encoder ? encoder.encode(text).length : text.length * 2;
}

/**
 * Fast size gate. UTF-8 uses at most 4 bytes per UTF-16 code unit, so short
 * notes skip the encoder entirely (important for a 500 KB paste).
 */
export function isNoteTooLarge(title: string, body: string): boolean {
  const characters = title.length + body.length + 2;
  if (characters * 4 <= MAX_NOTE_BYTES) return false;
  return noteByteLength(title, body) > MAX_NOTE_BYTES;
}

export function timeAgo(timestamp: unknown) {
  const minutes = Math.max(0, Math.floor((Date.now() - safeTimestamp(timestamp)) / 60000));
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `${hours}h ago` : `${Math.floor(hours / 24)}d ago`;
}
