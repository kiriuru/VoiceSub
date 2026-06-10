export function shouldThrottleAppendLog(
  throttleState: Map<string, number> | null,
  throttleKey: string,
  minGapMs: number,
  nowMs: number
): boolean {
  if (!throttleKey || !minGapMs || !throttleState) {
    return false;
  }
  const last = Number(throttleState.get(throttleKey) || 0);
  return Boolean(last && nowMs - last < minGapMs);
}

export function recordThrottledAppendLog(
  throttleState: Map<string, number> | null,
  throttleKey: string,
  nowMs: number
): void {
  if (!throttleState || !throttleKey) {
    return;
  }
  throttleState.set(throttleKey, nowMs);
}

export function recognitionStartBurstThrottle(
  reason: string,
  recognitionStartLogMinGapMs: number
): { gapMs: number; key: string | null } {
  const raw = String(reason || "")
    .trim()
    .toLowerCase()
    .replace(/-/g, "_");
  const burst = raw === "no_speech" || raw === "nospeech" || raw === "normal_onend";
  const gapMs = Math.max(500, Number(recognitionStartLogMinGapMs || 4200));
  if (!burst) {
    return { gapMs, key: null };
  }
  return { gapMs, key: `recognition-start:${raw}` };
}
