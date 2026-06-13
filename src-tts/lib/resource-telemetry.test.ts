import { describe, expect, it } from "vitest";
import {
  formatCompactBytes,
  formatHandleCount,
  findWatchedProcess,
  isResourceTelemetryWarning,
  type ResourceTelemetry,
} from "./resource-telemetry";

describe("resource-telemetry", () => {
  it("formats bytes and handles compactly", () => {
    expect(formatCompactBytes(4_242_714_624)).toBe("4.0G");
    expect(formatHandleCount(22_273)).toBe("22.3k");
  });

  it("flags high commit or handle counts", () => {
    expect(
      isResourceTelemetryWarning({
        pid: 1,
        name: "obs64.exe",
        handle_count: 22_273,
        commit_bytes: 1_000,
        working_set_bytes: 1_000,
      }),
    ).toBe(true);
  });

  it("finds watched processes by executable name", () => {
    const telemetry: ResourceTelemetry = {
      self_process: {
        pid: 10,
        name: "webview.exe",
        handle_count: 100,
        commit_bytes: 100,
        working_set_bytes: 100,
      },
      watched: [
        {
          pid: 20,
          name: "obs64.exe",
          handle_count: 1_000,
          commit_bytes: 2_000,
          working_set_bytes: 2_000,
        },
      ],
    };
    expect(findWatchedProcess(telemetry, "obs64.exe")?.pid).toBe(20);
  });
});
