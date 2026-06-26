import { describe, expect, it } from "vitest";
import { createBrowserAsrStateSeed } from "./session-state";
import { classifyRecognitionError } from "./recognition-error-logic";
import type { WorkerSpeechRecognitionErrorEvent } from "./speech-types";

function errorEvent(error: string, message = ""): WorkerSpeechRecognitionErrorEvent {
  return { error, message } as WorkerSpeechRecognitionErrorEvent;
}

describe("recognition-error-logic", () => {
  it("classifies no-speech and network", () => {
    const state = createBrowserAsrStateSeed();
    expect(classifyRecognitionError(errorEvent("no-speech"), undefined, state).kind).toBe("no_speech");
    expect(classifyRecognitionError(errorEvent("network"), undefined, state).kind).toBe("network");
  });

  it("retries phrases-not-supported once", () => {
    const state = createBrowserAsrStateSeed();
    expect(classifyRecognitionError(errorEvent("phrases-not-supported"), undefined, state).kind).toBe("phrases_retry");
  });

  it("retries language-not-supported before soft fallback", () => {
    const state = createBrowserAsrStateSeed({ webSpeechLanguageSoftFallbackUsed: false });
    expect(classifyRecognitionError(errorEvent("language-not-supported"), undefined, state).kind).toBe(
      "language_retry"
    );
  });

  it("maps permission errors to terminal", () => {
    const state = createBrowserAsrStateSeed();
    const classified = classifyRecognitionError(errorEvent("not-allowed"), undefined, state);
    expect(classified.kind).toBe("terminal_permission");
    expect(classified.terminalReason).toBe("permission_denied");
  });

  it("retries audio-capture instead of terminal stop", () => {
    const state = createBrowserAsrStateSeed();
    const classified = classifyRecognitionError(errorEvent("audio-capture"), undefined, state);
    expect(classified.kind).toBe("audio_capture");
  });

  it("maps exhausted language fallback to terminal language reason", () => {
    const state = createBrowserAsrStateSeed({ webSpeechLanguageSoftFallbackUsed: true });
    const classified = classifyRecognitionError(errorEvent("language-not-supported"), undefined, state);
    expect(classified.kind).toBe("terminal_language");
    expect(classified.terminalReason).toBe("language_not_supported");
  });
});
