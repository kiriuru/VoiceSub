import { describe, expect, it, vi } from "vitest";
import {
  flushUiConfigServerSyncForTests,
  publishUiConfigSync,
  uiConfigFromWsPayload,
} from "./ui-config-sync";

describe("uiConfigFromWsPayload", () => {
  it("extracts ui section from ws payload", () => {
    const payload = uiConfigFromWsPayload({
      ui: { theme: "dark", palette: { accent: "#FF4FB4" } },
    });
    expect(payload).toEqual({
      ui: { theme: "dark", palette: { accent: "#FF4FB4" } },
    });
  });

  it("returns null for invalid payloads", () => {
    expect(uiConfigFromWsPayload(null)).toBeNull();
    expect(uiConfigFromWsPayload({})).toBeNull();
    expect(uiConfigFromWsPayload({ ui: "dark" })).toBeNull();
  });
});

describe("publishUiConfigSync", () => {
  it("queues debounced server sync payload", () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true });
    vi.stubGlobal("fetch", fetchMock);

    publishUiConfigSync({
      ui: { theme: "dark", palette: { accent: "#FF4FB4" } },
    });
    expect(fetchMock).not.toHaveBeenCalled();

    flushUiConfigServerSyncForTests();
    expect(fetchMock).toHaveBeenCalledWith("/api/ui/sync", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        ui: { theme: "dark", palette: { accent: "#FF4FB4" } },
      }),
    });

    vi.unstubAllGlobals();
  });
});
