import { TTS_ACTIVITY_LOG_MAX } from "./activity-log";
import type { SpeechQueueItem } from "./types";

export type SpeechActivityKind = "source" | "translation" | "test";

export type SpeechActivityEntry = {
  id: string;
  lang: string;
  kind: SpeechActivityKind;
  slotId?: string;
  text: string;
};

export function speechActivityEntryFromQueueItem(
  item: SpeechQueueItem,
): SpeechActivityEntry {
  const source = String(item.source || "").trim();
  const lang = String(item.lang || "en").trim() || "en";
  if (source === "subtitle_source") {
    return {
      id: item.id,
      lang,
      kind: "source",
      text: item.text,
    };
  }
  if (source.startsWith("subtitle_")) {
    return {
      id: item.id,
      lang,
      kind: "translation",
      slotId: source.slice("subtitle_".length),
      text: item.text,
    };
  }
  return {
    id: item.id,
    lang,
    kind: "test",
    text: item.text,
  };
}

export function prependSpeechActivity(
  list: SpeechActivityEntry[],
  items: SpeechQueueItem[],
): SpeechActivityEntry[] {
  const batch = items
    .map(speechActivityEntryFromQueueItem)
    .filter((entry) => entry.text.trim());
  if (!batch.length) return list;
  // Keep speak order within a subtitle batch (source, then translations).
  return [...batch, ...list].slice(0, TTS_ACTIVITY_LOG_MAX);
}
