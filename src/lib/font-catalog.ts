export interface FontCatalogEntry {
  id: string;
  label: string;
  family: string;
  source: string;
  url?: string;
  filename?: string;
  format?: string;
}

export interface FontCatalog {
  project_fonts_dir: string;
  project_local: FontCatalogEntry[];
  fallback: FontCatalogEntry[];
  system?: FontCatalogEntry[];
}

const SYSTEM_FONTS_CACHE_KEY = "voicesub.system_fonts.v1";

export function mergeFontCatalogPreservingSystem(
  incoming: FontCatalog,
  previous?: FontCatalog | null,
): FontCatalog {
  const system = previous?.system?.length ? previous.system : incoming.system || [];
  return { ...incoming, system };
}

export function loadCachedSystemFonts(): FontCatalogEntry[] {
  try {
    const raw = localStorage.getItem(SYSTEM_FONTS_CACHE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as FontCatalogEntry[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

export function saveCachedSystemFonts(entries: FontCatalogEntry[]): void {
  localStorage.setItem(SYSTEM_FONTS_CACHE_KEY, JSON.stringify(entries));
}

export async function refreshSystemFonts(): Promise<FontCatalogEntry[]> {
  if (!("queryLocalFonts" in window)) {
    return loadCachedSystemFonts();
  }
  try {
    const fonts = await (window as Window & {
      queryLocalFonts?: () => Promise<Array<{ family: string; fullName?: string }>>;
    }).queryLocalFonts!();
    const seen = new Set<string>();
    const entries: FontCatalogEntry[] = [];
    for (const font of fonts) {
      const family = String(font.family || "").trim();
      if (!family || seen.has(family.toLowerCase())) continue;
      seen.add(family.toLowerCase());
      entries.push({
        id: `system-${family.toLowerCase().replace(/\s+/g, "-")}`,
        label: family,
        family: `"${family.replace(/"/g, "")}"`,
        source: "system",
      });
    }
    entries.sort((a, b) => a.label.localeCompare(b.label));
    saveCachedSystemFonts(entries);
    return entries;
  } catch {
    return loadCachedSystemFonts();
  }
}

export function fontOptions(catalog: FontCatalog | null): FontCatalogEntry[] {
  if (!catalog) return [];
  return [
    ...(catalog.project_local || []),
    ...(catalog.system || []),
    ...(catalog.fallback || []),
  ];
}

export function extractPrimaryFontFamily(chain: string): string {
  const str = String(chain || "").trim();
  if (!str) return "";
  const quoted = str.match(/"([^"]+)"/);
  if (quoted?.[1]) return `"${quoted[1].trim()}"`;
  const bare = str.split(",")[0]?.trim();
  return bare || "";
}

function fontFamilyKey(token: string): string {
  return extractPrimaryFontFamily(token).replace(/"/g, "").trim().toLowerCase();
}

/**
 * Replace only the primary face in a CSS font-family stack, keeping the rest
 * of the fallback chain (critical for Latin/JP + Cyrillic dual-script presets).
 */
export function replacePrimaryFontFamily(chain: string, newPrimaryFamily: string): string {
  const incoming = String(newPrimaryFamily || "").trim();
  if (!incoming) return String(chain || "");

  const rest = String(chain || "")
    .split(",")
    .slice(1)
    .map((part) => part.trim())
    .filter(Boolean);

  const primaryToken = incoming.includes(",")
    ? extractPrimaryFontFamily(incoming)
    : incoming;
  const normalizedPrimary = primaryToken.startsWith('"')
    ? primaryToken
    : `"${primaryToken.replace(/"/g, "")}"`;
  const primaryKey = fontFamilyKey(normalizedPrimary);

  const filteredRest = rest.filter((part) => {
    const key = fontFamilyKey(part);
    return Boolean(key) && key !== primaryKey;
  });

  return [normalizedPrimary, ...filteredRest].join(", ");
}
