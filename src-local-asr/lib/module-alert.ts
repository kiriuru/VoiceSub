export type ModuleAlertTone = "error" | "warn" | "info";

export function formatModuleAlertMessage(
  raw: string,
  networkLabel: string,
): string {
  if (raw === "network") return networkLabel;
  return raw
    .replace(/^dependency error:\s*/i, "")
    .replace(/^dependency check failed:\s*/i, "")
    .replace(/^inference error:\s*/i, "")
    .replace(/^manifest error:\s*/i, "")
    .replace(/^download failed:\s*/i, "")
    .replace(/^model error:\s*/i, "")
    .trim();
}

export function moduleAlertTitle(
  tone: ModuleAlertTone,
  labels: { error: string; warn: string; info: string },
): string {
  if (tone === "warn") return labels.warn;
  if (tone === "info") return labels.info;
  return labels.error;
}
