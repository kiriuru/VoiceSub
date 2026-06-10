type TraceFields = Record<string, unknown>;

function textFields(text: string): TraceFields {
  const preview = text.length > 80 ? `${text.slice(0, 80)}…` : text;
  return { text_len: text.length, preview };
}

export function ttsTrace(phase: string, event: string, fields: TraceFields = {}): void {
  void fetch("/api/logs/ui-trace", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      surface: "tts",
      phase,
      event,
      fields,
    }),
  }).catch(() => {});

  void fetch("/api/logs/client-event", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      channel: "tts",
      message: `${phase}.${event}`,
      source: "tts-window",
      details: fields,
    }),
  }).catch(() => {});

  if (import.meta.env.DEV) {
    console.debug(`[tts] ${phase}.${event}`, fields);
  }
}

export function ttsTraceText(
  phase: string,
  event: string,
  text: string,
  extra: TraceFields = {},
): void {
  ttsTrace(phase, event, { ...textFields(text), ...extra });
}
