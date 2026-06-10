import type { AsrManagerHost } from "./types";

export async function waitUntilDocumentVisibleForRecognition(
  manager: AsrManagerHost,
  options: { visibilityMaxMs?: number; focusMaxMs?: number; waitWindowFocus?: boolean } = {}
): Promise<boolean> {
  const visibilityMaxMs = Math.max(0, Number(options.visibilityMaxMs ?? 20000));
  const focusMaxMs = Math.max(0, Number(options.focusMaxMs ?? 6000));
  const waitFocus = Boolean(options.waitWindowFocus ?? false);

  if (document.hidden) {
    manager.appendLogInternal("document hidden; waiting for tab visibility before recognition start");
    await new Promise<void>((resolve) => {
      let done = false;
      const cleanup = () => {
        document.removeEventListener("visibilitychange", onVis);
        window.clearTimeout(timer);
      };
      const finish = () => {
        if (done) {
          return;
        }
        done = true;
        cleanup();
        resolve();
      };
      const onVis = () => {
        if (!document.hidden) {
          manager.appendLogInternal("tab became visible; continuing recognition start");
          finish();
        }
      };
      document.addEventListener("visibilitychange", onVis);
      const timer = window.setTimeout(() => {
        manager.appendLogInternal("visibility wait timed out; continuing recognition start anyway");
        finish();
      }, visibilityMaxMs);
    });
  }

  if (!manager.state.desiredRunning) {
    return false;
  }

  if (waitFocus && typeof document.hasFocus === "function" && !document.hasFocus()) {
    manager.appendLogInternal("window not focused; waiting briefly before recognition start");
    const startAt = manager.now();
    await new Promise<void>((resolve) => {
      const timer = window.setInterval(() => {
        if (!manager.state.desiredRunning) {
          window.clearInterval(timer);
          resolve();
          return;
        }
        if (document.hasFocus()) {
          manager.appendLogInternal("window focused; continuing recognition start");
          window.clearInterval(timer);
          resolve();
          return;
        }
        if (manager.now() - startAt >= focusMaxMs) {
          manager.appendLogInternal("focus wait timed out; continuing recognition start anyway");
          window.clearInterval(timer);
          resolve();
        }
      }, 80);
    });
  }

  return Boolean(manager.state.desiredRunning);
}
