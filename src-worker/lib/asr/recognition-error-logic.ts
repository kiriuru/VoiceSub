import type { BrowserAsrState, ClassifiedRecognitionError } from "./types";
import type { WorkerSpeechRecognitionErrorEvent } from "./speech-types";
import { webSpeechRecognitionPolicy } from "./web-speech-policy";

const TERMINAL_PERMISSION_ERRORS = ["not-allowed", "service-not-allowed", "audio-capture"];

function normalizeErrorKind(event: WorkerSpeechRecognitionErrorEvent): string {
  return String(event?.error || "")
    .trim()
    .toLowerCase() || "unknown";
}

function isPhrasesUnsupported(errorKind: string, policy = webSpeechRecognitionPolicy): boolean {
  return (
    (policy && typeof policy.isPhrasesNotSupportedError === "function" && policy.isPhrasesNotSupportedError(errorKind)) ||
    errorKind === "phrases-not-supported"
  );
}

function isLanguageUnsupported(errorKind: string, policy = webSpeechRecognitionPolicy): boolean {
  return (
    (policy && typeof policy.isLanguageNotSupportedError === "function" && policy.isLanguageNotSupportedError(errorKind)) ||
    errorKind === "language-not-supported"
  );
}

export function classifyRecognitionError(
  event: WorkerSpeechRecognitionErrorEvent,
  policy = webSpeechRecognitionPolicy,
  state: BrowserAsrState
): ClassifiedRecognitionError {
  const errorKind = normalizeErrorKind(event);
  const errorMessage = String(event?.message || "").trim();

  if (isPhrasesUnsupported(errorKind, policy)) {
    return { kind: "phrases_retry", errorKind, errorMessage };
  }
  if (isLanguageUnsupported(errorKind, policy) && !state.webSpeechLanguageSoftFallbackUsed) {
    return { kind: "language_retry", errorKind, errorMessage };
  }
  if (errorKind === "no-speech") {
    return { kind: "no_speech", errorKind, errorMessage };
  }
  if (errorKind === "network") {
    return { kind: "network", errorKind, errorMessage, logKey: "network_hint" };
  }
  if (errorKind === "aborted") {
    return { kind: "aborted", errorKind, errorMessage };
  }
  if (TERMINAL_PERMISSION_ERRORS.includes(errorKind)) {
    return {
      kind: "terminal_permission",
      errorKind,
      errorMessage,
      terminalReason: errorKind === "audio-capture" ? "audio_capture_recovery" : "permission_denied",
    };
  }
  if (isLanguageUnsupported(errorKind, policy)) {
    return { kind: "terminal_language", errorKind, errorMessage, terminalReason: "permission_denied" };
  }
  return { kind: "unknown", errorKind, errorMessage };
}

export function networkErrorHintMessages(translate: (key: string) => string): string {
  const translated = translate("browser_asr.network.hint");
  if (translated !== "browser_asr.network.hint") {
    return translated;
  }
  return "Web Speech network error: recognition service unreachable (VPN, firewall, DNS, proxy, blockers). Check connectivity; changing the browser microphone usually does not fix this.";
}
