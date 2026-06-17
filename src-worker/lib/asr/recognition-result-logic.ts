import type { WorkerSpeechRecognitionEvent } from "./speech-types";

export function parseRecognitionResultEvent(event: WorkerSpeechRecognitionEvent): {
  interimText: string;
  finalText: string;
  resultIndex: number;
} {
  let interimText = "";
  let finalText = "";
  const resultIndex = Number(event?.resultIndex || 0);
  const results = event?.results;
  if (!results || typeof results.length !== "number") {
    return { interimText, finalText, resultIndex };
  }
  for (let index = resultIndex; index < results.length; index += 1) {
    const result = results[index];
    if (!result) {
      continue;
    }
    const transcript = String(result?.[0]?.transcript || "").trim();
    if (!transcript) {
      continue;
    }
    if (result.isFinal) {
      finalText = `${finalText} ${transcript}`.trim();
    } else {
      interimText = `${interimText} ${transcript}`.trim();
    }
  }
  return { interimText, finalText, resultIndex };
}
