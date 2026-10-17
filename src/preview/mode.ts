import { isTauri } from "@tauri-apps/api/core";

/** True when the UI runs in a plain browser with the sample backend. */
export const isBrowserPreview = !isTauri();
