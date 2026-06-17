import type { TtsSpeechSettings } from "./types";

export type AppTranslationLine = {
  slot_id: string;
  enabled: boolean;
  target_lang: string;
  label?: string;
};

export type AppSpeechContext = {
  translationEnabled: boolean;
  sourceLang: string;
  lines: AppTranslationLine[];
};

export type TtsSpeakTarget = {
  id: string;
  kind: "source" | "translation";
  label: string;
  lang: string;
  slotId?: string;
};

const CANONICAL_SLOTS = [
  "translation_1",
  "translation_2",
  "translation_3",
  "translation_4",
  "translation_5",
] as const;

function slotNumber(slotId: string): number {
  const match = String(slotId).match(/(\d+)$/);
  return match?.[1] ? Number.parseInt(match[1], 10) : 0;
}

function normalizeLang(lang: string | undefined | null): string {
  const trimmed = String(lang || "")
    .trim()
    .toLowerCase();
  if (!trimmed) return "en";
  return trimmed.split("-")[0]?.split("_")[0] || "en";
}

function normalizeSlotId(slotId: string): string {
  return String(slotId || "").trim().toLowerCase();
}

export function getActiveTranslationLines(
  context: AppSpeechContext,
): AppTranslationLine[] {
  if (!context.translationEnabled) return [];
  return context.lines
    .filter((line) => line.enabled && normalizeSlotId(line.slot_id))
    .sort(
      (a, b) =>
        CANONICAL_SLOTS.indexOf(normalizeSlotId(a.slot_id) as (typeof CANONICAL_SLOTS)[number]) -
          CANONICAL_SLOTS.indexOf(normalizeSlotId(b.slot_id) as (typeof CANONICAL_SLOTS)[number]) ||
        slotNumber(a.slot_id) - slotNumber(b.slot_id),
    );
}

export { fetchAppSpeechContext } from "./app-settings";

export function lineDisplayLabel(line: AppTranslationLine): string {
  const lang = normalizeLang(line.target_lang).toUpperCase();
  const custom = String(line.label || "").trim();
  if (custom && custom.toUpperCase() !== lang) {
    return `${custom} · ${lang}`;
  }
  const n = slotNumber(line.slot_id);
  return n > 0 ? `Line ${n} · ${lang}` : `${lang}`;
}

export function buildSpeakTargets(
  context: AppSpeechContext,
  speech: TtsSpeechSettings,
): TtsSpeakTarget[] {
  const targets: TtsSpeakTarget[] = [];
  const sourceLang = normalizeLang(context.sourceLang);

  if (speech.speak_source) {
    targets.push({
      id: "source",
      kind: "source",
      label: `Source · ${sourceLang.toUpperCase()}`,
      lang: sourceLang,
    });
  }

  const activeLines = getActiveTranslationLines(context);
  const selectedSlots = new Set(
    (speech.translation_slots || []).map((slot) => normalizeSlotId(slot)),
  );
  const speakAllTranslations =
    speech.speak_translations && selectedSlots.size === 0;

  for (const line of activeLines) {
    const slotId = normalizeSlotId(line.slot_id);
    if (!speakAllTranslations && !selectedSlots.has(slotId)) continue;
    if (!speech.speak_translations) continue;
    targets.push({
      id: slotId,
      kind: "translation",
      label: lineDisplayLabel(line),
      lang: normalizeLang(line.target_lang),
      slotId,
    });
  }

  return targets;
}

export function reconcileSpeechSlots(
  speech: TtsSpeechSettings,
  activeLines: AppTranslationLine[],
): TtsSpeechSettings {
  const activeSlotIds = activeLines.map((line) => normalizeSlotId(line.slot_id));
  const activeSet = new Set(activeSlotIds);
  const current = (speech.translation_slots || []).map(normalizeSlotId);
  const pruned = current.filter((slot) => activeSet.has(slot));

  let translationSlots = pruned;
  if (speech.speak_translations && translationSlots.length === 0 && activeSlotIds.length > 0) {
    translationSlots = [...activeSlotIds];
  }

  return {
    ...speech,
    translation_slots: translationSlots,
    speak_translations: speech.speak_translations && activeSlotIds.length > 0
      ? translationSlots.length > 0
      : false,
  };
}

export function isTranslationSlotSelected(
  speech: TtsSpeechSettings,
  slotId: string,
): boolean {
  if (!speech.speak_translations) return false;
  const normalized = normalizeSlotId(slotId);
  const slots = (speech.translation_slots || []).map(normalizeSlotId);
  if (slots.length === 0) return true;
  return slots.includes(normalized);
}

export function toggleTranslationSlot(
  speech: TtsSpeechSettings,
  slotId: string,
  checked: boolean,
  activeLines: AppTranslationLine[],
): TtsSpeechSettings {
  const activeSlotIds = activeLines.map((line) => normalizeSlotId(line.slot_id));
  const normalized = normalizeSlotId(slotId);
  let slots = (speech.translation_slots || []).map(normalizeSlotId);

  if (slots.length === 0 && speech.speak_translations) {
    slots = [...activeSlotIds];
  }

  if (checked) {
    if (!slots.includes(normalized)) slots.push(normalized);
  } else {
    slots = slots.filter((slot) => slot !== normalized);
  }

  return {
    ...speech,
    speak_translations: slots.length > 0,
    translation_slots: slots,
  };
}

export const TEST_TARGET_STORAGE_KEY = "voicesub.tts.test_target";

export function loadTestTargetId(targets: TtsSpeakTarget[]): string {
  if (typeof localStorage === "undefined") {
    return targets[0]?.id || "source";
  }
  const saved = localStorage.getItem(TEST_TARGET_STORAGE_KEY);
  if (saved && targets.some((target) => target.id === saved)) {
    return saved;
  }
  return targets[0]?.id || "source";
}

export function saveTestTargetId(id: string) {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(TEST_TARGET_STORAGE_KEY, id);
}
