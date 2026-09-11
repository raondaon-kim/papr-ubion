// Static platform detection for the webview's host OS.
//
// Tauri's window decorations differ per platform: on macOS the title bar is
// overlaid (Overlay + hiddenTitle + custom trafficLightPosition) and the
// frontend draws its own drag region; on Windows and Linux the native title
// bar handles dragging, and the extra 38px-tall .titlebar would be a dead
// strip above the content. We branch on this constant so the mac-only chrome
// stays out of the way everywhere else.
export const isMac =
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad|iPod/.test(navigator.platform || navigator.userAgent || "");

// The Vite development server can render the UI in an ordinary browser, while
// the desktop application supplies Tauri's IPC bridge. Keep native-only calls
// behind this check so a localhost visual preview does not crash before the
// shell renders.
export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// The primary modifier glyph for the current platform. Shown in <kbd> chips,
// command-palette hints, the shortcuts cheat sheet — anywhere a label sits
// next to (or instead of) the actual key.
export const modKey = isMac ? "⌘" : "Ctrl";

// Render a full modifier+key combo the way each platform spells it: macOS
// concatenates the glyph (⌘K), Windows / Linux use the "Ctrl+K" form.
export const modCombo = (key: string) => (isMac ? `⌘${key}` : `Ctrl+${key}`);
