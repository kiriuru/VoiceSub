/** Phone portrait logical size (iPhone 14 class). */
export const COMPACT_WINDOW = {
  width: 390,
  height: 844,
  minWidth: 360,
  minHeight: 640,
  maxWidth: 430,
  maxHeight: 932,
} as const;

export const STANDARD_WINDOW = {
  width: 1280,
  height: 900,
  minWidth: 960,
  minHeight: 640,
} as const;

/** Resize Tauri main window: compact = phone portrait, standard = desktop dashboard. */
export async function applyDashboardWindowSize(compact: boolean): Promise<void> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("set_dashboard_layout", { compact });
    return;
  } catch {
    // Not in Tauri shell — try direct window API (some dev setups).
  }

  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const { LogicalSize } = await import("@tauri-apps/api/dpi");
    const window = getCurrentWindow();
    if (compact) {
      await window.setSize(new LogicalSize(COMPACT_WINDOW.width, COMPACT_WINDOW.height));
      await window.setMinSize(new LogicalSize(COMPACT_WINDOW.minWidth, COMPACT_WINDOW.minHeight));
      await window.setMaxSize(new LogicalSize(COMPACT_WINDOW.maxWidth, COMPACT_WINDOW.maxHeight));
    } else {
      await window.setMaxSize(null);
      await window.setMinSize(new LogicalSize(STANDARD_WINDOW.minWidth, STANDARD_WINDOW.minHeight));
      await window.setSize(new LogicalSize(STANDARD_WINDOW.width, STANDARD_WINDOW.height));
    }
    await window.center();
  } catch {
    // Browser-only dev — no window chrome.
  }
}
