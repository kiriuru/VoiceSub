import { reportTtsWebviewActivity } from "./tts-ipc";
import { ttsTrace } from "./tts-trace";

type WakeLockSentinelLike = {
  released: boolean;
  release: () => Promise<void>;
  addEventListener: (type: "release", listener: () => void) => void;
};

export type TtsKeepaliveContext = {
  runtimeActive: boolean;
  ttsEnabled: boolean;
  enginesBusy: boolean;
};

let wakeLockSentinel: WakeLockSentinelLike | null = null;
let wakeLockRetryTimer: ReturnType<typeof setTimeout> | null = null;
let started = false;
let lastContext: TtsKeepaliveContext = {
  runtimeActive: false,
  ttsEnabled: false,
  enginesBusy: false,
};

function hasWakeLockSupport(): boolean {
  return typeof navigator !== "undefined" && Boolean(navigator.wakeLock?.request);
}

export function shouldHoldWakeLock(context: TtsKeepaliveContext): boolean {
  return context.runtimeActive && context.ttsEnabled && context.enginesBusy;
}

function contextsEqual(a: TtsKeepaliveContext, b: TtsKeepaliveContext): boolean {
  return (
    a.runtimeActive === b.runtimeActive &&
    a.ttsEnabled === b.ttsEnabled &&
    a.enginesBusy === b.enginesBusy
  );
}

function clearWakeLockRetryTimer(): void {
  if (wakeLockRetryTimer) {
    clearTimeout(wakeLockRetryTimer);
    wakeLockRetryTimer = null;
  }
}

async function acquireWakeLock(reason: string): Promise<void> {
  if (!hasWakeLockSupport() || document.hidden) {
    return;
  }
  if (wakeLockSentinel && !wakeLockSentinel.released) {
    return;
  }
  try {
    const sentinel = (await navigator.wakeLock.request("screen")) as WakeLockSentinelLike;
    wakeLockSentinel = sentinel;
    ttsTrace("keepalive", "wake_lock_acquired", { reason });
    sentinel.addEventListener("release", () => {
      if (wakeLockSentinel !== sentinel) return;
      wakeLockSentinel = null;
      ttsTrace("keepalive", "wake_lock_released", {});
      if (!shouldHoldWakeLock(lastContext)) return;
      clearWakeLockRetryTimer();
      wakeLockRetryTimer = setTimeout(() => {
        void acquireWakeLock("reacquire-after-release");
      }, 500);
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    ttsTrace("keepalive", "wake_lock_failed", { reason, message });
  }
}

async function releaseWakeLock(reason: string): Promise<void> {
  clearWakeLockRetryTimer();
  const sentinel = wakeLockSentinel;
  wakeLockSentinel = null;
  if (!sentinel) return;
  try {
    await sentinel.release();
    ttsTrace("keepalive", "wake_lock_stopped", { reason });
  } catch {
    // best effort
  }
}

function syncWakeLock(context: TtsKeepaliveContext): void {
  if (shouldHoldWakeLock(context)) {
    void acquireWakeLock("engines-busy");
    return;
  }
  void releaseWakeLock("engines-idle");
}

function notifyRust(context: TtsKeepaliveContext): void {
  void reportTtsWebviewActivity(
    context.runtimeActive,
    context.ttsEnabled,
    context.enginesBusy,
  ).catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    ttsTrace("keepalive", "webview_activity_error", { message });
  });
}

function onVisibilityChange(): void {
  const hidden = document.hidden;
  ttsTrace("keepalive", "visibility_changed", { hidden });
  if (hidden) {
    clearWakeLockRetryTimer();
    wakeLockRetryTimer = setTimeout(() => {
      if (!document.hidden) {
        syncWakeLock(lastContext);
      }
    }, 1500);
    notifyRust(lastContext);
    return;
  }
  syncWakeLock(lastContext);
  notifyRust(lastContext);
}

/** Install visibility hooks once; wake lock and WebView power follow activity updates. */
export function startTtsKeepalive(): void {
  if (started || typeof window === "undefined") return;
  started = true;
  document.addEventListener("visibilitychange", onVisibilityChange);
  ttsTrace("keepalive", "started", {
    wake_lock_supported: hasWakeLockSupport(),
    visibility_state: document.visibilityState,
  });
}

export function stopTtsKeepalive(): void {
  if (!started || typeof window === "undefined") return;
  started = false;
  document.removeEventListener("visibilitychange", onVisibilityChange);
  lastContext = {
    runtimeActive: false,
    ttsEnabled: false,
    enginesBusy: false,
  };
  void releaseWakeLock("tts-module-stop");
  notifyRust(lastContext);
  ttsTrace("keepalive", "stopped", {});
}

/** Update activity; acquire wake lock and notify Rust only when TTS engines are busy. */
export function updateTtsKeepalive(context: TtsKeepaliveContext): void {
  if (!started || typeof window === "undefined") return;
  if (contextsEqual(context, lastContext)) return;
  lastContext = context;
  syncWakeLock(context);
  notifyRust(context);
  ttsTrace("keepalive", "context_updated", context);
}
