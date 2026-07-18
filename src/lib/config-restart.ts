import { t, type LocaleCode } from "./i18n";
import type { ConfigPayload } from "./types";

export function getRestartRequiredReasons(
  previousPayload: ConfigPayload,
  nextPayload: ConfigPayload,
): string[] {
  const reasons: string[] = [];

  // logging.full_enabled is applied live on settings save via apply_logging_preferences.
  if (
    String(previousPayload.asr?.browser?.recognition_language || "ru-RU") !==
    String(nextPayload.asr?.browser?.recognition_language || "ru-RU")
  ) {
    reasons.push("config.restart_reason.web_speech_language");
  }
  return reasons;
}

export function formatReasonList(reasons: string[], locale?: LocaleCode): string {
  const tr = (key: string, params?: Record<string, string>) => t(key, params, locale);
  if (reasons.length <= 1) return reasons[0] || "";
  if (reasons.length === 2) {
    const first = reasons[0] ?? "";
    const second = reasons[1] ?? "";
    return tr("format.list.two", { first, second });
  }
  return tr("format.list.many", {
    head: reasons.slice(0, -1).join(", "),
    last: reasons[reasons.length - 1] ?? "",
  });
}

export function buildSaveStatusMessage(
  liveApplied: boolean,
  restartReasonKeys: string[],
  runtime: { running?: boolean; is_running?: boolean } | null | undefined,
  locale?: LocaleCode,
): string {
  const tr = (key: string, params?: Record<string, string>) => t(key, params, locale);
  if (!restartReasonKeys.length) {
    return liveApplied ? tr("config.save.applied_immediately") : tr("config.save.saved_locally");
  }
  const subject = formatReasonList(
    restartReasonKeys.map((key) => tr(key)),
    locale,
  );
  const restartLabel = runtime?.running || runtime?.is_running
    ? tr("config.save.restart_after_stop_start")
    : tr("config.save.restart_on_next_start");
  if (liveApplied) {
    return tr("config.save.applied_with_restart", { subject, restartLabel });
  }
  return tr("config.save.local_with_restart", { subject, restartLabel });
}
