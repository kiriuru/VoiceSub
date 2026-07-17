import { describe, expect, it } from "vitest";
import {
  formatLocalAsrExecutionProvider,
  formatLocalAsrModelLabel,
  isLocalAsrCudaProvider,
  readLocalModuleBadgeSnapshot,
} from "./local-asr-labels";

describe("local-asr-labels", () => {
  const tr = (key: string) => key;

  it("formats execution provider badges", () => {
    expect(formatLocalAsrExecutionProvider("cuda")).toBe("CUDA");
    expect(formatLocalAsrExecutionProvider("cpu")).toBe("CPU");
    expect(isLocalAsrCudaProvider("cuda")).toBe(true);
    expect(isLocalAsrCudaProvider("cpu")).toBe(false);
  });

  it("falls back to family and variant ids when i18n keys are missing", () => {
    expect(formatLocalAsrModelLabel(tr, "parakeet_tdt", "int8")).toBe("parakeet_tdt · int8");
  });

  it("reads camelCase local module status from runtime payload", () => {
    const snap = readLocalModuleBadgeSnapshot({
      ready: true,
      phase: "ready",
      executionProvider: "cpu",
      activeExecutionProvider: "cuda",
      activeModelFamily: "parakeet_tdt",
      activeModelVariant: "int8",
    });
    expect(snap?.executionProvider).toBe("cuda");
    expect(snap?.activeModelVariant).toBe("int8");
  });
});
