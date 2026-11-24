import { useRef } from "react";
import {
  createNoteSession,
  type NoteSession,
  type NoteSessionOptions,
} from "../notes/noteSession";

/**
 * Creates the single Notes session for the whole app. It lives in Notch, not
 * in NotesView, so tab switches and panel closes only unmount the editor, not
 * its text, save queue, caret or scroll position.
 */
export function useNoteSession(options: NoteSessionOptions): NoteSession {
  const optionsRef = useRef(options);
  optionsRef.current = options;
  const sessionRef = useRef<NoteSession | null>(null);

  if (!sessionRef.current) {
    sessionRef.current = createNoteSession({
      getSettings: () => optionsRef.current.getSettings(),
      persist: (lastId, lastEditedMs) =>
        optionsRef.current.persist(lastId, lastEditedMs),
      service: optionsRef.current.service,
      now: optionsRef.current.now,
      autosaveMs: optionsRef.current.autosaveMs,
    });
  }

  return sessionRef.current;
}
