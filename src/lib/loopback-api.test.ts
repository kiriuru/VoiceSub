/**
 * @vitest-environment happy-dom
 */
import { afterEach, describe, expect, it } from "vitest";
import {
  LOOPBACK_TOKEN_HEADER,
  loopbackApiHeaders,
  loopbackApiToken,
  withLoopbackAuth,
} from "./loopback-api";

describe("loopback-api", () => {
  afterEach(() => {
    delete window.__VOICESUB_API_TOKEN__;
  });

  it("reads injected token from trusted HTML", () => {
    window.__VOICESUB_API_TOKEN__ = "session-token-123";
    expect(loopbackApiToken()).toBe("session-token-123");
    const headers = new Headers(loopbackApiHeaders());
    expect(headers.get(LOOPBACK_TOKEN_HEADER)).toBe("session-token-123");
  });

  it("throws when token is missing for protected API calls", () => {
    expect(() => loopbackApiHeaders()).toThrow(/loopback API token is missing/i);
    expect(() => withLoopbackAuth({ method: "POST" })).toThrow(/loopback API token is missing/i);
  });
});
