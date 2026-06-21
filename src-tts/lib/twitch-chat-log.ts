import { prependActivityLog, TTS_ACTIVITY_LOG_MAX } from "./activity-log";
import type { TwitchChatMessage } from "./types";

export type TwitchChatLogEntry = TwitchChatMessage & {
  /** Stable key for `{#each}` — always unique per rendered row. */
  logKey: string;
  /** Stable key for skipping duplicate chat deliveries. */
  dedupeKey: string;
};

export function twitchChatDedupeKey(
  message: TwitchChatMessage & Record<string, unknown>,
  uniqueFallback?: string,
): string {
  const id = String(message.id ?? "").trim();
  if (id) {
    return `id:${id}`;
  }
  const seq = message.event_sequence;
  if (typeof seq === "number" && Number.isFinite(seq)) {
    return `seq:${seq}`;
  }
  const user = String(message.user ?? "").trim();
  const channel = String(message.channel ?? "").trim();
  const text = String(message.text ?? "").trim();
  const created = message.created_at_ms;
  if (typeof created === "number" && Number.isFinite(created)) {
    return `fallback:${user}|${channel}|${text}|${created}`;
  }
  // No stable identity (no id, sequence, or timestamp). Collapsing on user|channel|text
  // alone would silently drop genuine repeated messages, so fall back to a per-row unique
  // key when one is available — duplicate IPC delivery without any identifier is far less
  // likely than a user legitimately repeating themselves (review MED#13).
  if (uniqueFallback) {
    return `unique:${uniqueFallback}`;
  }
  return `fallback:${user}|${channel}|${text}`;
}

export function prependTwitchChatLog(
  list: TwitchChatLogEntry[],
  message: TwitchChatMessage & Record<string, unknown>,
  logKey: string,
  max = TTS_ACTIVITY_LOG_MAX,
): TwitchChatLogEntry[] {
  const dedupeKey = twitchChatDedupeKey(message, logKey);
  if (list.some((entry) => entry.dedupeKey === dedupeKey)) {
    return list;
  }
  const entry: TwitchChatLogEntry = { ...message, logKey, dedupeKey };
  return prependActivityLog(list, entry, max);
}
