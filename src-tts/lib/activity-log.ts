export const TTS_ACTIVITY_LOG_MAX = 5;

export function prependActivityLog<T>(
  list: T[],
  item: T,
  max = TTS_ACTIVITY_LOG_MAX,
): T[] {
  return [item, ...list].slice(0, max);
}
