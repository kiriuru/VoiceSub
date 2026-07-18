import { describe, expect, it } from "vitest";

import { normalizeDiagnosticsPayload } from "./diagnostics-normalizer";

describe("normalizeDiagnosticsPayload", () => {
  it("preserves local_module from previous snapshot when update omits it", () => {
    const previous = {
      provider: "browser_google",
      local_module: { ready: true, phase: "ready" },
      active_mode: "local_parakeet",
    };
    const next = normalizeDiagnosticsPayload(
      { provider: "browser_google", browser_worker: { worker_connected: true } },
      previous,
    );
    expect(next.local_module).toEqual({ ready: true, phase: "ready" });
    expect(next.active_mode).toBe("local_parakeet");
    expect(next.browser_worker).toEqual({ worker_connected: true });
  });

  it("prefers local_module from the incoming payload", () => {
    const next = normalizeDiagnosticsPayload(
      { local_module: { ready: false, phase: "loading" } },
      { local_module: { ready: true, phase: "ready" } },
    );
    expect(next.local_module).toEqual({ ready: false, phase: "loading" });
  });
});
