import { describe, expect, it } from "vitest";
import { normalizeConfigPayload } from "./config-normalize";
import type { ConfigPayload } from "./types";

describe("normalizeConfigPayload SST parity", () => {
  it("maps overlay compact preset to stacked plus compact flag", () => {
    const out = normalizeConfigPayload({
      overlay: { preset: "compact" },
    } as ConfigPayload);
    expect(out.overlay?.preset).toBe("stacked");
    expect(out.overlay?.compact).toBe(true);
  });

  it("maps invalid worker_launch_browser to auto", () => {
    const out = normalizeConfigPayload({
      asr: { browser: { worker_launch_browser: "safari" } },
    } as ConfigPayload);
    expect(out.asr?.browser?.worker_launch_browser).toBe("auto");
  });

  it("maps chromium worker_launch_browser to auto", () => {
    const out = normalizeConfigPayload({
      asr: { browser: { worker_launch_browser: "chromium" } },
    } as ConfigPayload);
    expect(out.asr?.browser?.worker_launch_browser).toBe("auto");
  });

  it("falls back removed mymemory provider to google_translate_v2", () => {
    const out = normalizeConfigPayload({
      translation: {
        provider: "mymemory",
        lines: [{ provider: "mymemory", target_lang: "en", slot_id: "translation_1" }],
      },
    } as ConfigPayload);
    expect(out.translation?.provider).toBe("google_translate_v2");
    expect(out.translation?.lines?.[0]?.provider).toBe("google_translate_v2");
  });

  it("fills translation provider_settings with SST defaults", () => {
    const out = normalizeConfigPayload({} as ConfigPayload);
    expect(out.translation?.provider_settings?.deepl?.api_url).toBe(
      "https://api-free.deepl.com/v2/translate",
    );
    expect(out.translation?.provider_settings?.azure_translator?.endpoint).toBe(
      "https://api.cognitive.microsofttranslator.com",
    );
    expect(out.translation?.provider_settings?.google_cloud_translation_v3?.location).toBe("global");
  });

  it("migrates google v3 UI aliases to canonical provider_settings keys", () => {
    const out = normalizeConfigPayload({
      translation: {
        provider_settings: {
          google_cloud_translation_v3: {
            api_key: "token-1",
            endpoint: "proj-1",
            region: "eu",
          },
        },
      },
    } as ConfigPayload);
    const v3 = out.translation?.provider_settings?.google_cloud_translation_v3;
    expect(v3?.access_token).toBe("token-1");
    expect(v3?.project_id).toBe("proj-1");
    expect(v3?.location).toBe("eu");
    expect(v3?.api_key).toBeUndefined();
  });

  it("round-trips ui.language and clears invalid locales", () => {
    const saved = normalizeConfigPayload({
      ui: { language: "ru" },
    } as ConfigPayload);
    expect(saved.ui?.language).toBe("ru");

    const invalid = normalizeConfigPayload({
      ui: { language: "de" },
    } as ConfigPayload);
    expect(invalid.ui?.language).toBe("");
  });

  it("clamps invalid obs_closed_captions.output_mode to disabled", () => {
    const out = normalizeConfigPayload({
      obs_closed_captions: { output_mode: "bogus_mode" },
    } as ConfigPayload);
    expect(out.obs_closed_captions?.output_mode).toBe("disabled");
  });

  it("keeps valid obs_closed_captions.output_mode", () => {
    const out = normalizeConfigPayload({
      obs_closed_captions: { output_mode: "translation_2" },
    } as ConfigPayload);
    expect(out.obs_closed_captions?.output_mode).toBe("translation_2");
  });

  // Deprecated keys: still normalized/synced for legacy configs (no runtime effect).
  it("syncs lifecycle pause/hard_max with asr.realtime and completed TTLs", () => {
    const out = normalizeConfigPayload({
      subtitle_lifecycle: {
        completed_source_ttl_ms: 2500,
        completed_translation_ttl_ms: 7000,
        pause_to_finalize_ms: 420,
        hard_max_phrase_ms: 6200,
      },
    } as ConfigPayload);
    expect(out.subtitle_lifecycle?.completed_source_ttl_ms).toBe(2500);
    expect(out.subtitle_lifecycle?.completed_translation_ttl_ms).toBe(7000);
    expect(out.subtitle_lifecycle?.completed_block_ttl_ms).toBe(7000);
    expect(out.subtitle_lifecycle?.pause_to_finalize_ms).toBe(420);
    expect(out.subtitle_lifecycle?.hard_max_phrase_ms).toBe(6200);
    expect(out.asr?.realtime?.finalization_hold_ms).toBe(420);
    expect(out.asr?.realtime?.max_segment_ms).toBe(6200);
  });

  it("falls back lifecycle pause/hard_max to asr.realtime when lifecycle keys missing", () => {
    const out = normalizeConfigPayload({
      asr: {
        realtime: {
          finalization_hold_ms: 380,
          max_segment_ms: 6100,
        },
      },
    } as ConfigPayload);
    expect(out.subtitle_lifecycle?.pause_to_finalize_ms).toBe(380);
    expect(out.subtitle_lifecycle?.hard_max_phrase_ms).toBe(6100);
    expect(out.asr?.realtime?.finalization_hold_ms).toBe(380);
    expect(out.asr?.realtime?.max_segment_ms).toBe(6100);
  });
});
