/** Default idle window before Notes opens a fresh draft (0 disables it). */
export const DEFAULT_NOTES_NEW_AFTER_MINUTES = 720;

/**
 * Decide which note the Notes tab should show when it opens.
 *
 * Returns the id of the last note to reopen, or `null` for a fresh draft.
 * Positional arguments keep the owner-specified call shape explicit.
 *
 * Rules:
 * - New note pressed: always a fresh draft.
 * - No remembered id, or its file is gone: fresh draft.
 * - `newAfterMinutes === 0`: never expire, so reopen the last note.
 * - Last edit older than the window: fresh draft.
 */
export function chooseNoteOnOpen(
  nowMs: number,
  lastEditedMs: number,
  lastId: string | null,
  fileExists: boolean,
  newAfterMinutes: number,
  newNotePressed: boolean,
): string | null {
  if (newNotePressed) return null;
  if (!lastId || !fileExists) return null;

  // 0 (or an invalid/negative value) means "never open a fresh note by time".
  if (!Number.isFinite(newAfterMinutes) || newAfterMinutes <= 0) return lastId;

  // An unknown edit time cannot prove the note is recent.
  if (!Number.isFinite(lastEditedMs) || lastEditedMs <= 0) return null;

  const ageMs = nowMs - lastEditedMs;
  if (ageMs > newAfterMinutes * 60_000) return null;
  return lastId;
}
