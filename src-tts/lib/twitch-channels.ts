import type { TwitchTtsSettings } from "./types";

export const TWITCH_MAX_CHANNELS = 5;

export function normalizeChannelLogin(raw: string): string {
  return raw.trim().replace(/^#+/, "").toLowerCase();
}

export function resolveChannelRows(twitch: TwitchTtsSettings): string[] {
  const fromList = (twitch.channels ?? [])
    .map((entry) => entry.trim())
    .filter(Boolean);
  if (fromList.length > 0) {
    return fromList.slice(0, TWITCH_MAX_CHANNELS);
  }
  const legacy = twitch.channel?.trim();
  return legacy ? [legacy] : [""];
}

export function channelsFromRows(rows: string[]): {
  channels: string[];
  channel: string;
} {
  const channels: string[] = [];
  for (const raw of rows) {
    const login = normalizeChannelLogin(raw);
    if (!login || channels.includes(login)) continue;
    channels.push(login);
    if (channels.length >= TWITCH_MAX_CHANNELS) break;
  }
  return {
    channels,
    channel: channels[0] ?? "",
  };
}
