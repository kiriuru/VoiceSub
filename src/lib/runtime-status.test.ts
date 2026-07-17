import { describe, expect, it } from "vitest";
import {
  buildRuntimeConnectionChips,
  resolveObsChipStatus,
  resolveRuntimePhase,
} from "./runtime-status";
import type { RuntimeStatus } from "./types";

describe("runtime-status", () => {
  it("resolves runtime phase from status fallback", () => {
    expect(resolveRuntimePhase({ status: "listening" })).toBe("listening");
    expect(resolveRuntimePhase({ phase: "translating" })).toBe("translating");
  });

  it("maps OBS diagnostics to chip status", () => {
    expect(resolveObsChipStatus({ enabled: true, output_mode: "native" }, {}).status).toBe("ready");
    expect(resolveObsChipStatus({ last_error: "auth failed" }, {}).status).toBe("error");
    expect(resolveObsChipStatus({}, {}).status).toBe("disabled");
  });

  it("builds connection chips for live strip", () => {
    const runtime: RuntimeStatus = {
      running: true,
      phase: "listening",
      asr: {
        active_mode: "browser_google",
        diagnostics: { browser_worker: { worker_connected: true } },
      },
    };
    const chips = buildRuntimeConnectionChips(runtime, true, { enabled: true, output_mode: "overlay" });
    expect(chips.workerConnected).toBe(true);
    expect(chips.showBrowserWorkerChip).toBe(true);
    expect(chips.showLocalAsrChip).toBe(false);
    expect(chips.obsStatus).toBe("ready");
    expect(chips.obsLabel).toBe("overlay");
  });

  it("uses local ASR chips and status message when active mode is local_parakeet", () => {
    const runtime: RuntimeStatus = {
      running: true,
      phase: "listening",
      status_message: "Loading Parakeet TDT int8 (CUDA)…",
      asr: {
        active_mode: "local_parakeet",
        diagnostics: { browser_worker: { worker_connected: false } },
      },
    };
    const chips = buildRuntimeConnectionChips(runtime, true, {});
    expect(chips.showLocalAsrChip).toBe(true);
    expect(chips.showBrowserWorkerChip).toBe(false);
    expect(chips.asrSourceConnected).toBe(true);
    expect(chips.statusMessage).toContain("Loading Parakeet");
  });
});
