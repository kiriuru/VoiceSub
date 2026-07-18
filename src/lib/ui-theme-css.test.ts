import { describe, expect, it } from "vitest";
import {
  applyUiColorSchemeToDocument,
  applyUiFontToDocument,
  hexToRgbTriplet,
} from "./ui-theme-css";

describe("hexToRgbTriplet", () => {
  it("converts hex to space-separated rgb", () => {
    expect(hexToRgbTriplet("#6cc7ff")).toBe("108 199 255");
  });
});

function mockDocument(): Document {
  const props = new Map<string, string>();
  const dataset: Record<string, string> = {};
  const root = {
    dataset,
    style: {
      setProperty(name: string, value: string) {
        props.set(name, value);
      },
      removeProperty(name: string) {
        props.delete(name);
      },
      getPropertyValue(name: string) {
        return props.get(name) || "";
      },
    },
  };
  return { documentElement: root } as unknown as Document;
}

describe("applyUiColorSchemeToDocument", () => {
  it("sets data-ui-theme and color-scheme", () => {
    const doc = mockDocument();
    applyUiColorSchemeToDocument("light", doc);
    expect(doc.documentElement.dataset.uiTheme).toBe("light");
    expect(doc.documentElement.style.getPropertyValue("color-scheme")).toBe("light");
  });
});

describe("applyUiFontToDocument", () => {
  it("sets --font-ui when font family is provided", () => {
    const doc = mockDocument();
    applyUiFontToDocument('"Segoe UI", sans-serif', doc);
    expect(doc.documentElement.style.getPropertyValue("--font-ui")).toBe(
      '"Segoe UI", sans-serif',
    );
  });

  it("clears --font-ui when font family is empty", () => {
    const doc = mockDocument();
    doc.documentElement.style.setProperty("--font-ui", '"Arial", sans-serif');
    applyUiFontToDocument("", doc);
    expect(doc.documentElement.style.getPropertyValue("--font-ui")).toBe("");
  });

  it("trims whitespace before applying", () => {
    const doc = mockDocument();
    applyUiFontToDocument('  "Meiryo", sans-serif  ', doc);
    expect(doc.documentElement.style.getPropertyValue("--font-ui")).toBe(
      '"Meiryo", sans-serif',
    );
  });
});
