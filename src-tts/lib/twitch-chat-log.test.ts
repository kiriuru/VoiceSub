import { describe, expect, it } from "vitest";

import {
  prependTwitchChatLog,
  twitchChatDedupeKey,
  type TwitchChatLogEntry,
} from "./twitch-chat-log";
import type { TwitchChatMessage } from "./types";

const sampleMessage = (): TwitchChatMessage => ({
  id: "abc-123",
  user: "viewer",
  display_name: "Viewer",
  text: "hello chat",
  speak_text: "viewer, hello chat",
  channel: "#channel",
  language: "en",
  is_mod: false,
  is_subscriber: false,
});

describe("twitchChatDedupeKey", () => {
  it("prefers twitch message id", () => {
    expect(twitchChatDedupeKey(sampleMessage())).toBe("id:abc-123");
  });

  it("falls back to runtime event_sequence", () => {
    expect(
      twitchChatDedupeKey({
        ...sampleMessage(),
        id: "",
        event_sequence: 42,
      }),
    ).toBe("seq:42");
  });

  it("uses a per-row unique key when no stable identity exists", () => {
    const msg = { ...sampleMessage(), id: "" } as TwitchChatMessage &
      Record<string, unknown>;
    delete (msg as Record<string, unknown>).event_sequence;
    delete (msg as Record<string, unknown>).created_at_ms;
    expect(twitchChatDedupeKey(msg, "chat-7")).toBe("unique:chat-7");
  });
});

describe("prependTwitchChatLog", () => {
  it("skips duplicate deliveries with the same twitch id", () => {
    const first = prependTwitchChatLog([], sampleMessage(), "chat-0");
    const second = prependTwitchChatLog(first, sampleMessage(), "chat-1");
    expect(second).toHaveLength(1);
    expect(second[0]?.logKey).toBe("chat-0");
  });

  it("keeps distinct messages", () => {
    let list: TwitchChatLogEntry[] = [];
    list = prependTwitchChatLog(list, sampleMessage(), "chat-0");
    list = prependTwitchChatLog(
      list,
      { ...sampleMessage(), id: "def-456", text: "second" },
      "chat-1",
    );
    expect(list).toHaveLength(2);
  });

  it("does not collapse identical-text messages that lack any stable identity", () => {
    const make = () => {
      const msg = {
        user: "viewer",
        display_name: "Viewer",
        text: "lol",
        speak_text: "viewer, lol",
        channel: "#channel",
        language: "en",
        is_mod: false,
        is_subscriber: false,
      } as unknown as TwitchChatMessage & Record<string, unknown>;
      return msg;
    };
    let list: TwitchChatLogEntry[] = [];
    list = prependTwitchChatLog(list, make(), "chat-0");
    list = prependTwitchChatLog(list, make(), "chat-1");
    expect(list).toHaveLength(2);
  });
});
