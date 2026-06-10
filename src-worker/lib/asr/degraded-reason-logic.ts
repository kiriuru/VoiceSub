import type { BrowserAsrState } from "./types";

export function resolveDegradedReason(state: BrowserAsrState): string | null {
  if (state.terminalDegradedReason) {
    return state.terminalDegradedReason;
  }
  if (state.visibilityDegraded) {
    return "document_hidden";
  }
  if (state.socketDegraded) {
    return "websocket_disconnected";
  }
  if (state.healthDegradedReason) {
    return state.healthDegradedReason;
  }
  return null;
}
