import { buildSaveStatusMessage } from "./config-restart";
import { t, type LocaleCode } from "./i18n";

export type SaveStatusState =
  | { tone: "default" }
  | { tone: "busy" }
  | { tone: "error"; message: string }
  | {
      tone: "success" | "warn";
      liveApplied: boolean;
      restartReasonKeys: string[];
    };

export function formatSaveStatusDisplay(
  state: SaveStatusState,
  runtime: { running?: boolean; is_running?: boolean } | null | undefined,
  locale?: LocaleCode,
): string {
  switch (state.tone) {
    case "default":
      return t("save.status.default", undefined, locale);
    case "busy":
      return t("common.loading", undefined, locale);
    case "error":
      return state.message;
    case "success":
    case "warn":
      return buildSaveStatusMessage(
        state.liveApplied,
        state.restartReasonKeys,
        runtime,
        locale,
      );
  }
}
