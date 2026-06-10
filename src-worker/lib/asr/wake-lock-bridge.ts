import type { AsrManagerHost } from "./types";

export function hasWakeLockSupport(): boolean {
  return typeof navigator !== "undefined" && Boolean(navigator?.wakeLock?.request);
}

export function clearWakeLockRetryTimer(manager: AsrManagerHost): void {
  if (manager._wakeLockRetryTimer) {
    window.clearTimeout(manager._wakeLockRetryTimer);
    manager._wakeLockRetryTimer = null;
  }
}

export async function acquireWakeLock(manager: AsrManagerHost, reason?: string): Promise<boolean> {
  if (!hasWakeLockSupport()) {
    manager.state.wakeLockActive = false;
    return false;
  }
  if (document.hidden) {
    clearWakeLockRetryTimer(manager);
    manager._wakeLockRetryTimer = window.setTimeout(
      () => void acquireWakeLock(manager, "retry-after-visibility"),
      1500
    );
    return false;
  }
  if (manager._wakeLockSentinel && !manager._wakeLockSentinel.released) {
    manager.state.wakeLockActive = true;
    return true;
  }
  try {
    const sentinel = await navigator.wakeLock.request("screen");
    if (!sentinel) {
      manager.state.wakeLockActive = false;
      return false;
    }
    manager._wakeLockSentinel = sentinel;
    manager.state.wakeLockActive = true;
    if (!manager._wakeLockBound) {
      manager._wakeLockBound = true;
    }
    sentinel.addEventListener("release", () => {
      if (manager._wakeLockSentinel === sentinel) {
        manager._wakeLockSentinel = null;
        manager.state.wakeLockActive = false;
        if (manager.state.desiredRunning) {
          clearWakeLockRetryTimer(manager);
          manager._wakeLockRetryTimer = window.setTimeout(
            () => void acquireWakeLock(manager, "re-acquire-after-release"),
            500
          );
        }
      }
    });
    manager.appendLogInternal(`screen wake lock acquired (${reason || "start"})`);
    return true;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error || "");
    manager.state.wakeLockActive = false;
    if (message) {
      manager.appendLogInternal(`screen wake lock acquisition failed: ${message}`);
    }
    return false;
  }
}

export async function releaseWakeLock(manager: AsrManagerHost, reason?: string): Promise<void> {
  clearWakeLockRetryTimer(manager);
  const sentinel = manager._wakeLockSentinel;
  manager._wakeLockSentinel = null;
  manager.state.wakeLockActive = false;
  if (!sentinel) {
    return;
  }
  try {
    await sentinel.release();
    manager.appendLogInternal(`screen wake lock released (${reason || "stop"})`);
  } catch {
    // best effort
  }
}
