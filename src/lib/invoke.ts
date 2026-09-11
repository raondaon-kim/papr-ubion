// `invoke` / `channel` that work both inside the desktop app and in a plain
// browser tab served by `pnpm dev`. In Tauri they are the real IPC primitives;
// in a browser they route to the read-only preview bridge.

import { invoke as tauriInvoke, Channel as TauriChannel } from "@tauri-apps/api/core";
import { isTauri } from "./platform";
import { previewInvoke } from "./previewBridge";

export const invoke: typeof tauriInvoke = isTauri
  ? tauriInvoke
  : (cmd, args) => previewInvoke(cmd, (args ?? {}) as Record<string, unknown>);

/** A message channel for streaming commands. Constructing a real Tauri
 *  `Channel` needs `__TAURI_INTERNALS__`, so the browser gets an inert stand-in
 *  with the same `onmessage` shape (nothing will ever stream into it). */
export function channel<T>(onmessage?: (m: T) => void): TauriChannel<T> {
  if (isTauri) {
    const ch = new TauriChannel<T>();
    if (onmessage) ch.onmessage = onmessage;
    return ch;
  }
  return { id: -1, onmessage: onmessage ?? (() => {}) } as unknown as TauriChannel<T>;
}
