/**
 * `FLOAT_DEBUG=1` stage logs for the window handshake. The Rust side returns
 * the flag from `ui_ready`; until then lines are buffered so the first
 * listener registrations still appear in the log with their real timestamps.
 */
type Sink = (line: string) => void;

let enabled: boolean | null = null;
let sink: Sink | null = null;
const pending: string[] = [];

/** uiService points this at `logWebview` (level 3 = info). */
export function setDebugSink(next: Sink): void {
  sink = next;
}

function emit(line: string): void {
  console.info(line);
  sink?.(line);
}

export function debugLog(message: string): void {
  if (enabled === false) return;
  const line = `${message} t=${Math.round(performance.now())}ms`;
  if (enabled === true) {
    emit(line);
    return;
  }
  // Keep the startup burst bounded; ui_ready flushes this in order.
  if (pending.length < 100) pending.push(line);
}

export function setDebugEnabled(next: boolean): void {
  enabled = next;
  if (!next) {
    pending.length = 0;
    return;
  }
  for (const line of pending.splice(0)) emit(line);
}
