import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");

function readPreviewSource(): string {
  return readFileSync(
    join(root, "src/lib/components/SubtitleOutputPreview.svelte"),
    "utf8",
  );
}

describe("dashboard preview contracts (SubtitleOutputPreview)", () => {
  it("disposes preview renderer on destroy and empty payload", () => {
    const source = readPreviewSource();
    expect(source).toContain("disposeRenderContainer");
    expect(source).toContain("onDestroy");
    expect(source).toContain("renderer.disposeRenderContainer?.(previewEl)");
  });

  it("renders through SubtitleStyleRenderer when preview payload exists", () => {
    const source = readPreviewSource();
    expect(source).toContain("SubtitleStyleRenderer");
    expect(source).toContain("renderer.render(previewEl, previewPayload");
    expect(source).toContain("buildPreviewPayload");
  });
});
