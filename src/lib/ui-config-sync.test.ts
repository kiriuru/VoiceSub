import { afterEach, describe, expect, it, vi } from "vitest";
import {
  flushUiConfigServerSyncForTests,
  publishUiConfigSync,
  uiConfigFromWsPayload,
  uiPresentationSignature,
} from "./ui-config-sync";
import { LOOPBACK_TOKEN_HEADER } from "./loopback-api";

describe("uiPresentationSignature", () => {
  it("ignores non-presentation config fields", () => {
    const a = uiPresentationSignature({
      ui: { theme: "dark", language: "en", layout: "standard" },
      translation: { enabled: true },
    });
    const b = uiPresentationSignature({
      ui: { theme: "dark", language: "en", layout: "standard" },
      translation: { enabled: false },
    });
    expect(a).toBe(b);
  });

  it("changes when theme/locale/font/layout/palette change", () => {
    const base = uiPresentationSignature({
      ui: { theme: "dark", language: "en", layout: "standard", font_family: "Segoe UI" },
    });
    expect(
      uiPresentationSignature({
        ui: { theme: "light", language: "en", layout: "standard", font_family: "Segoe UI" },
      }),
    ).not.toBe(base);
  });
});

describe("uiConfigFromWsPayload", () => {
  it("extracts ui section from ws payload", () => {
    const payload = uiConfigFromWsPayload({
      ui: { theme: "dark", font_family: '"Segoe UI", sans-serif', palette: { accent: "#FF4FB4" } },
    });
    expect(payload).toEqual({
      ui: { theme: "dark", font_family: '"Segoe UI", sans-serif', palette: { accent: "#FF4FB4" } },
    });
  });

  it("returns null for invalid payloads", () => {
    expect(uiConfigFromWsPayload(null)).toBeNull();
    expect(uiConfigFromWsPayload({})).toBeNull();
    expect(uiConfigFromWsPayload({ ui: "dark" })).toBeNull();
  });
});

describe("publishUiConfigSync", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("queues debounced server sync payload with loopback auth header", async () => {
    vi.stubGlobal("window", {
      __VOICESUB_API_TOKEN__: "test-loopback-token",
    });
    const fetchMock = vi.fn().mockResolvedValue({ ok: true });
    vi.stubGlobal("fetch", fetchMock);

    publishUiConfigSync({
      ui: { theme: "dark", palette: { accent: "#FF4FB4" } },
    });
    expect(fetchMock).not.toHaveBeenCalled();

    await flushUiConfigServerSyncForTests();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/ui/sync");
    expect(init.method).toBe("POST");
    const headers = new Headers(init.headers);
    expect(headers.get("Content-Type")).toBe("application/json");
    expect(headers.get(LOOPBACK_TOKEN_HEADER)).toBe("test-loopback-token");
    expect(init.body).toBe(
      JSON.stringify({
        ui: { theme: "dark", palette: { accent: "#FF4FB4" } },
      }),
    );
  });

  it("skips server sync when loopback token is missing", async () => {
    vi.stubGlobal("window", {});
    const fetchMock = vi.fn().mockResolvedValue({ ok: true });
    vi.stubGlobal("fetch", fetchMock);

    publishUiConfigSync({
      ui: { theme: "dark", palette: { accent: "#FF4FB4" } },
    });
    await flushUiConfigServerSyncForTests();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
