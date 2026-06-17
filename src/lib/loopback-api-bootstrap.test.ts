/**
 * @vitest-environment happy-dom
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn<[string], Promise<unknown>>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string) => invokeMock(cmd),
}));

describe("loopback-api bootstrap / Tauri invoke", () => {
  beforeEach(() => {
    vi.resetModules();
    invokeMock.mockReset();
    delete window.__VOICESUB_API_TOKEN__;
  });

  afterEach(() => {
    delete window.__VOICESUB_API_TOKEN__;
  });

  async function loadLoopbackModules() {
    const loopback = await import("./loopback-api");
    const client = await import("./loopback-api-client");
    loopback.__resetLoopbackApiTokenForTests();
    return { ...loopback, apiFetch: client.apiFetch };
  }

  it("Tauri invoke supplies token for authenticated apiFetch", async () => {
    invokeMock.mockImplementation(async (cmd) => {
      if (cmd === "get_loopback_api_token") {
        return "invoke-session-token";
      }
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const fetchMock = vi.fn().mockResolvedValue(new Response("{}", { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    const { initLoopbackApiToken, LOOPBACK_TOKEN_HEADER, apiFetch } = await loadLoopbackModules();
    await initLoopbackApiToken();
    await apiFetch("/api/version");

    expect(fetchMock).toHaveBeenCalledOnce();
    const init = fetchMock.mock.calls[0]?.[1] as RequestInit | undefined;
    const headers = new Headers(init?.headers);
    expect(headers.get(LOOPBACK_TOKEN_HEADER)).toBe("invoke-session-token");
  });

  it("rejects protected headers until initLoopbackApiToken settles (redirect race)", async () => {
    let releaseInvoke!: () => void;
    const invokeGate = new Promise<string>((resolve) => {
      releaseInvoke = () => resolve("late-token");
    });
    invokeMock.mockImplementation(async (cmd) => {
      if (cmd === "get_loopback_api_token") {
        return invokeGate;
      }
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const { initLoopbackApiToken, loopbackApiHeaders } = await loadLoopbackModules();
    const initPromise = initLoopbackApiToken();

    expect(() => loopbackApiHeaders()).toThrow(/loopback API token is missing/i);

    releaseInvoke();
    await expect(initPromise).resolves.toBe("late-token");
    expect(new Headers(loopbackApiHeaders()).get("x-voicesub-token")).toBe("late-token");
  });

  it("prefers injected HTML token before Tauri invoke", async () => {
    window.__VOICESUB_API_TOKEN__ = "html-injected-token";
    invokeMock.mockRejectedValue(new Error("invoke should not run"));

    const { initLoopbackApiToken, loopbackApiToken } = await loadLoopbackModules();
    await initLoopbackApiToken();

    expect(loopbackApiToken()).toBe("html-injected-token");
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("relative /api URLs resolve against dashboard HTTP entry after redirect", () => {
    const entry = "http://127.0.0.1:8765/";
    expect(new URL("/api/health", entry).href).toBe("http://127.0.0.1:8765/api/health");
    expect(new URL("/ws/events", entry).href).toBe("http://127.0.0.1:8765/ws/events");
  });
});
