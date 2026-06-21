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
