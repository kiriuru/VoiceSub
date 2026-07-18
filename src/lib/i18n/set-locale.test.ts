import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getLocale, setLocale } from "./index";

describe("setLocale", () => {
  beforeEach(() => {
    const target = new EventTarget();
    vi.stubGlobal(
      "window",
      Object.assign(target, {
        dispatchEvent: target.dispatchEvent.bind(target),
        addEventListener: target.addEventListener.bind(target),
        removeEventListener: target.removeEventListener.bind(target),
      }),
    );
    setLocale("en");
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("does not re-emit when locale is unchanged (prevents Local ASR feedback spin)", () => {
    setLocale("ru");
    let events = 0;
    const onChanged = () => {
      events += 1;
    };
    window.addEventListener("sst:locale-changed", onChanged);
    try {
      setLocale("ru");
      setLocale("ru");
      expect(events).toBe(0);
      expect(getLocale()).toBe("ru");

      setLocale("en");
      expect(events).toBe(1);
      expect(getLocale()).toBe("en");
    } finally {
      window.removeEventListener("sst:locale-changed", onChanged);
    }
  });

  it("stops CustomEvent feedback when a listener calls setLocale with the same code", () => {
    let nestedCalls = 0;
    const onChanged = () => {
      nestedCalls += 1;
      // Old Local ASR bug: listener always called setLocale(detail.locale).
      setLocale("ja");
    };
    window.addEventListener("sst:locale-changed", onChanged);
    try {
      setLocale("ja");
      expect(getLocale()).toBe("ja");
      // One event from en→ja; nested setLocale("ja") must not emit again.
      expect(nestedCalls).toBe(1);
    } finally {
      window.removeEventListener("sst:locale-changed", onChanged);
    }
  });
});
