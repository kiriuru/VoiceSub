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
    expect(chips.obsStatus).toBe("ready");
    expect(chips.obsLabel).toBe("overlay");
  });
});
