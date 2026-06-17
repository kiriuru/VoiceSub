/** Marks the document when running inside the Tauri desktop shell (Mica / transparent window). */
export function isTauriDesktopShell(win: Window = window): boolean {
  const w = win as Window & { __TAURI_INTERNALS__?: unknown; __TAURI__?: unknown };
  return Boolean(w.__TAURI_INTERNALS__ ?? w.__TAURI__);
}

export function markDesktopShell(doc: Document = document): void {
  if (typeof window === "undefined" || typeof document === "undefined") return;
  if (!isTauriDesktopShell()) return;

  doc.documentElement.dataset.shell = "tauri";
}
