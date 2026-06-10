import { normalizeConfigPayload } from "../../src/lib/config-normalize";
import type { ConfigPayload } from "../../src/lib/types";

export interface SourceTextReplacementPair {
  source: string;
  target: string;
}

export interface SourceTextReplacementSettings {
  enabled: boolean;
  include_builtin: boolean;
  case_insensitive: boolean;
  whole_words: boolean;
  pairs: SourceTextReplacementPair[];
}

/** Shared profanity filter flags from the main app (custom pairs are not synced). */
export function sourceTextReplacementFromConfig(
  config: ConfigPayload,
): SourceTextReplacementSettings {
  const normalized = normalizeConfigPayload(config);
  const block = (normalized.source_text_replacement || {}) as Record<string, unknown>;
  const wholeWordOnly = block.whole_word_only === true;
  const wholeWordsLegacy = block.whole_words !== false;

  return {
    enabled: false,
    include_builtin:
      (block.include_builtin_profanity ?? block.include_builtin) !== false,
    case_insensitive: block.case_insensitive !== false,
    whole_words: wholeWordOnly || (block.whole_word_only === undefined && wholeWordsLegacy),
    pairs: [],
  };
}

export function profanityIncludeBuiltinFromConfig(config: ConfigPayload): boolean {
  return sourceTextReplacementFromConfig(config).include_builtin;
}
