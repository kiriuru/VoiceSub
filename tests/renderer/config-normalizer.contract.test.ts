import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");

function readNormalizerSource(): string {
  return readFileSync(join(root, "src/lib/config-normalize.ts"), "utf8");
}

describe("config-normalize contracts", () => {
  it("normalizes overlay compact flag and preset", () => {
    const source = readNormalizerSource();
    expect(source).toContain("overlay.compact = compact");
    expect(source).toContain('"single", "dual-line", "stacked"');
    expect(source).toContain('if (preset === "compact")');
  });

  it("maps chromium worker_launch_browser to auto", () => {
    const source = readNormalizerSource();
    expect(source).toContain('launchBrowser === "chromium"');
    expect(source).toContain('launchBrowser = "auto"');
  });

  it("normalizes ui.language against supported locale set", () => {
    const source = readNormalizerSource();
    for (const locale of ["en", "ru", "ja", "ko", "zh"]) {
      expect(source).toContain(`"${locale}"`);
    }
    expect(source).toContain('["en", "ru", "ja", "ko", "zh"].includes(lang)');
  });

  it("falls back unknown translation providers through PROVIDERS map", () => {
    const source = readNormalizerSource();
    expect(source).toContain("normalizeTranslationProvider");
    expect(source).toContain("provider in PROVIDERS ? provider : fallback");
  });
});
