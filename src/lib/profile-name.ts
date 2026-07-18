/** Profile name rules mirrored from `voicesub_config::ProfileStore::profile_path`. */

const WINDOWS_RESERVED = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/i;
const INVALID_CHARS = /[<>:"|?*\u0000-\u001f]/;

export function normalizeProfileName(name: string): string {
  return String(name || "").trim();
}

export function isValidProfileName(name: string): boolean {
  const raw = normalizeProfileName(name);
  if (!raw || raw === "." || raw === "..") return false;
  if (raw.includes("..") || raw.includes("/") || raw.includes("\\")) return false;
  if (INVALID_CHARS.test(raw)) return false;
  if (WINDOWS_RESERVED.test(raw)) return false;
  if (raw.endsWith(".") || raw.endsWith(" ")) return false;
  return true;
}
