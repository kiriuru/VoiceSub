import { describe, expect, it } from "vitest";
import {
  TWITCH_MAX_CHANNELS,
  channelsFromRows,
  normalizeChannelLogin,
  resolveChannelRows,
} from "./twitch-channels";
import type { TwitchTtsSettings } from "./types";

describe("twitch-channels", () => {
  it("normalizes login names", () => {
    expect(normalizeChannelLogin("  #Foo  ")).toBe("foo");
  });

  it("prefers channels array over legacy channel field", () => {
    const twitch = {
      channel: "legacy",
      channels: ["Alpha", "#beta"],
    } as TwitchTtsSettings;
    expect(resolveChannelRows(twitch)).toEqual(["Alpha", "#beta"]);
  });

  it("falls back to legacy channel", () => {
    const twitch = { channel: "solo", channels: [] } as TwitchTtsSettings;
    expect(resolveChannelRows(twitch)).toEqual(["solo"]);
  });

  it("dedupes and caps channel rows", () => {
    const rows = ["#one", "ONE", "two", "three", "four", "five", "six"];
    expect(channelsFromRows(rows)).toEqual({
      channels: ["one", "two", "three", "four", "five"],
      channel: "one",
    });
    expect(channelsFromRows(rows).channels).toHaveLength(TWITCH_MAX_CHANNELS);
  });
});
