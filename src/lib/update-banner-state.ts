import type { VersionInfo } from "./types";

/** True when the user dismissed the banner for this exact latest version. */
export function isUpdateBannerDismissedForVersion(
  latestVersion: string,
  dismissedVersion: string,
): boolean {
  const latest = latestVersion.trim();
  if (!latest) {
    return false;
  }
  return dismissedVersion.trim() === latest;
}

/** Whether the dashboard update banner should render. */
export function shouldShowUpdateBanner(
  versionInfo: VersionInfo | null | undefined,
  bannerDismissed: boolean,
): boolean {
  if (bannerDismissed || !versionInfo?.sync?.update_available) {
    return false;
  }
  const latest = versionInfo.sync.latest_known_version?.trim() || "";
  return Boolean(latest);
}
