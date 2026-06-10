import { describe, expect, it } from "vitest";
import { buildProviderOptionGroups, getProviderFieldLabel } from "./translation-helpers";
import { t } from "./i18n";

describe("translation-helpers", () => {
  it("builds grouped provider options for all supported providers", () => {
    const groups = buildProviderOptionGroups();
    const ids = groups.flatMap((group) => group.providers.map((item) => item.id));
    expect(ids).toContain("google_translate_v2");
    expect(ids).toContain("openai");
    expect(ids).toContain("free_web_translate");
    expect(ids.length).toBe(13);
    for (const group of groups) {
      expect(group.labelKey).toMatch(/^translation\.provider_group\./);
      expect(t(group.labelKey, undefined, "ru")).not.toBe(group.labelKey);
    }
  });

  it("maps google v3 UI fields to dedicated labels", () => {
    const label = getProviderFieldLabel("google_cloud_translation_v3", "api_key", (key) => key);
    expect(label).toBe("translation.field.google_v3.api_key");
  });
});
