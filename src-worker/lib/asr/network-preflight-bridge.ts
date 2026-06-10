import type { AsrManagerHost } from "./types";
import { registerNetworkErrorBurst } from "./restart-timing-logic";
import { releaseWakeLock } from "./wake-lock-bridge";

const PREFLIGHT_URL = "https://www.google.com/generate_204";

export function registerNetworkErrorForPreflight(manager: AsrManagerHost): void {
  const now = manager.now();
  if (registerNetworkErrorBurst(manager.state, now, manager.timingLimits())) {
    void runNetworkPreflight(manager, "network-burst-threshold");
  }
}

export async function runNetworkPreflight(manager: AsrManagerHost, reason?: string): Promise<boolean> {
  manager.state.networkPreflightInFlight = true;
  manager.state.lastNetworkPreflightAtMs = manager.now();
  manager.appendLogInternal(`network preflight probe started (${reason || "network-burst"})`);
  manager.emitWorkerStatus("network-preflight-start");
  const controller = typeof AbortController === "function" ? new AbortController() : null;
  const timeoutId = controller
    ? window.setTimeout(() => controller.abort(), manager.networkPreflightTimeoutMs)
    : null;
  let ok = false;
  try {
    const response = await fetch(PREFLIGHT_URL, {
      method: "GET",
      mode: "no-cors",
      cache: "no-store",
      credentials: "omit",
      referrerPolicy: "no-referrer",
      signal: controller ? controller.signal : undefined,
    });
    ok = Boolean(response);
  } catch {
    ok = false;
  } finally {
    if (timeoutId) {
      window.clearTimeout(timeoutId);
    }
  }
  manager.state.lastNetworkPreflightOk = ok;
  manager.state.networkPreflightInFlight = false;
  manager.appendLogInternal(`network preflight probe result: ${ok ? "reachable" : "unreachable"}`);
  manager.emitWorkerStatus(ok ? "network-preflight-ok" : "network-preflight-failed");
  if (!ok) {
    manager.state.desiredRunning = false;
    manager.state.pendingStart = false;
    manager.clearAllTimersInternal();
    manager.setSupervisorStateInternal("fatal");
    manager.setTerminalDegradedReasonInternal("recognition_network_unreachable");
    manager.setStatusInternal(manager.translate("browser_asr.network.status_unreachable"));
    manager.appendLogInternal(manager.translate("browser_asr.network.preflight_failed_log"));
    await releaseWakeLock(manager, "network-preflight-failed");
    manager.emitWorkerStatus("terminal-network-unreachable");
    return false;
  }
  return true;
}
