import type { RecognitionSettings } from "./types";
import type { WorkerSpeechRecognition } from "./speech-types";

export function shouldEnableRecognitionOverlap(settings: RecognitionSettings | null | undefined): boolean {
  if (!settings || typeof settings !== "object") {
    return false;
  }
  if (settings.overlap_recognition_sessions === false) {
    return false;
  }
  if (settings.overlap_recognition_sessions === true) {
    return true;
  }
  return settings.continuous === false;
}

export function stripChromeOnDeviceHints(recognition: WorkerSpeechRecognition | null | undefined): void {
  if (!recognition || typeof recognition !== "object") {
    return;
  }
  try {
    (recognition as WorkerSpeechRecognition & { processLocally?: boolean }).processLocally = false;
  } catch {
    // ignore
  }
  try {
    const phrases = (recognition as WorkerSpeechRecognition & { phrases?: unknown[] }).phrases;
    if (phrases && typeof phrases.pop === "function") {
      while (phrases.length > 0) {
        phrases.pop();
      }
    } else {
      (recognition as WorkerSpeechRecognition & { phrases?: unknown[] }).phrases = [];
    }
  } catch {
    try {
      delete (recognition as WorkerSpeechRecognition & { phrases?: unknown }).phrases;
    } catch {
      // ignore
    }
  }
}

export function isPhrasesNotSupportedError(kind: string): boolean {
  return String(kind || "")
    .trim()
    .toLowerCase() === "phrases-not-supported";
}

export function isLanguageNotSupportedError(kind: string): boolean {
  return String(kind || "")
    .trim()
    .toLowerCase() === "language-not-supported";
}

export const webSpeechRecognitionPolicy = {
  shouldEnableRecognitionOverlap,
  stripChromeOnDeviceHints,
  isPhrasesNotSupportedError,
  isLanguageNotSupportedError,
};
