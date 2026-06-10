import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const source = readFileSync(join(root, "bin/overlay/shared/js/subtitle-style.js"), "utf8");

// subtitle-style.js is a browser IIFE that attaches to window.SubtitleStyleRenderer.
new Function(source)();
