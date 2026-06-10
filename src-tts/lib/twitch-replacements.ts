import type { TwitchReplacement } from "./types";

/** One mapping per line: `from => to` or `from = to` */
export function parseReplacementLines(text: string): TwitchReplacement[] {
  const out: TwitchReplacement[] = [];
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const split = line.includes("=>") ? line.split("=>") : line.split("=");
    if (split.length < 2) continue;
    const from = split[0]?.trim() ?? "";
    const to = split.slice(1).join("=").trim();
    if (from && to) out.push({ from, to });
  }
  return out;
}

export function formatReplacementLines(entries: TwitchReplacement[] | undefined): string {
  return (entries ?? [])
    .map((entry) => `${entry.from} => ${entry.to}`)
    .join("\n");
}
