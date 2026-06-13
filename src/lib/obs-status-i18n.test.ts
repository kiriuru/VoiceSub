import { describe, expect, it } from "vitest";
import en from "./i18n/locales/en.json";
import ja from "./i18n/locales/ja.json";
import ko from "./i18n/locales/ko.json";
import ru from "./i18n/locales/ru.json";
import zh from "./i18n/locales/zh.json";
import {
  formatObsCaptionError,
  formatObsCcRuntimeStatus,
  formatObsNativeCaptionStatus,
  resolveObsErrorCode,
} from "./obs-status-i18n";

const tr = (key: string, vars?: Record<string, string>) => {
  let text = (en as Record<string, string>)[key] || key;
  if (vars) {
    for (const [name, value] of Object.entries(vars)) {
      text = text.replaceAll(`{${name}}`, String(value ?? ""));
    }
  }
  return text;
};

describe("obs status i18n", () => {
  it("resolves stable error codes from legacy English messages", () => {
    expect(resolveObsErrorCode("connection_refused")).toBe("connection_refused");
    expect(
      resolveObsErrorCode(
        "OBS captions unavailable: obs websocket io error: IO error: connection refused (os error 10061).",
      ),
    ).toBe("connection_refused");
  });

  it("resolves connection refused from Russian Windows IO text", () => {
    expect(
      resolveObsErrorCode(
        "obs websocket io error: IO error: Подключение не установлено, т.к. конечный компьютер отверг запрос на подключение. (os error 10061)",
      ),
    ).toBe("connection_refused");
  });

  it("formats errors using locale catalog keys", () => {
    expect(formatObsCaptionError("connection_refused", tr)).toBe(
      en["obs.cc.error.connection_refused"],
    );
  });

  it("formats native caption status codes", () => {
    expect(formatObsNativeCaptionStatus("stream_active", tr)).toBe(en["obs.cc.native.stream_active"]);
  });

  it("formats tools runtime obs cc line without raw backend text", () => {
    const line = formatObsCcRuntimeStatus(
      {
        connection_state: "error",
        last_error:
          "OBS captions unavailable: obs websocket io error: IO error: Подключение не установлено, т.к. конечный компьютер отверг запрос на подключение. (os error 10061)",
      },
      tr,
    );
    expect(line).toContain(en["obs.cc.connection_state.error"]);
    expect(line).toContain(en["obs.cc.error.connection_refused"]);
    expect(line).not.toContain("OBS captions unavailable");
    expect(line).not.toContain("Подключение не установлено");
  });

  it("defines tools.profiles.name_label in all UI locales", () => {
    for (const catalog of [en, ru, ja, ko, zh] as Record<string, string>[]) {
      expect(catalog["tools.profiles.name_label"]).toBeTruthy();
    }
  });

  it("defines obs error and native keys in all UI locales", () => {
    const required = [
      "obs.cc.error.connection_refused",
      "obs.cc.error.generic",
      "obs.cc.native.stream_active",
      "obs.cc.status.error",
      "obs.stream.reconnecting",
      "tools.runtime.obs_cc_status",
      "tools.profiles.name_label",
    ];
    for (const catalog of [en, ru, ja, ko, zh] as Record<string, string>[]) {
      for (const key of required) {
        expect(catalog[key], `missing ${key}`).toBeTruthy();
      }
      expect(catalog["obs.cc.status.error"]).toContain("{error}");
    }
  });
});
