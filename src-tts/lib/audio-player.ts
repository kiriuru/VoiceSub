export function isNativePlaybackMode(
  playbackMode: string | undefined | null,
): boolean {
  return String(playbackMode || "").trim().toLowerCase() === "native";
}

export type SpeechChannel = "speech" | "twitch";
