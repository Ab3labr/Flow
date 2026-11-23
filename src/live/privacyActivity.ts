import type { PrivacyState } from "../services/types.ts";
import { LIVE_PRIORITIES } from "./priorities.ts";
import type { LiveActivity, LiveLeading } from "./types.ts";

const MIC_COLOR = "#ff9f0a";
const CAMERA_COLOR = "#30d158";

/** Labels are fixed by the approved design; the app name is the only variable. */
export function privacyLabel(state: PrivacyState): string {
  const mic = state.kinds.includes("microphone");
  const camera = state.kinds.includes("camera");
  if (mic && camera) return "Camera and mic";
  if (camera) return "Camera";
  return "Microphone";
}

function privacyLeading(state: PrivacyState): LiveLeading {
  const mic = state.kinds.includes("microphone");
  const camera = state.kinds.includes("camera");
  if (mic && camera) {
    return {
      type: "icons",
      icons: [
        { name: "mic", color: MIC_COLOR },
        { name: "camera", color: CAMERA_COLOR },
      ],
    };
  }
  if (camera) return { type: "icon", name: "camera", color: CAMERA_COLOR };
  return { type: "icon", name: "mic", color: MIC_COLOR };
}

/** A new usage start becomes a 4 s transient compact-bar slot. */
export function privacyActivity(state: PrivacyState): LiveActivity {
  const mic = state.kinds.includes("microphone");
  return {
    id: "privacy",
    kind: "privacy",
    mode: "transient",
    ttlMs: 4_000,
    priority: LIVE_PRIORITIES.privacy,
    accent: mic ? MIC_COLOR : CAMERA_COLOR,
    leading: privacyLeading(state),
    label: privacyLabel(state),
    trailing: { type: "text", value: state.app },
    hover: "none",
    maxWidth: 260,
  };
}
