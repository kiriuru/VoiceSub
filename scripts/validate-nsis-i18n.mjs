#!/usr/bin/env node
/**
 * Strict NSIS installer i18n audit for VoiceSub.
 *
 * 1. Every custom $(LangString) used in installer.nsi/hooks.nsh exists and is
 *    non-empty in each bundled language file.
 * 2. Windows UI LANGID -> installer language mapping matches installer.nsi logic.
 * 3. For representative system locales (incl. de-DE fallback), the resolved
 *    language file contains all required strings (no empty UI on any locale).
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");

const tauriConf = JSON.parse(
  fs.readFileSync(path.join(root, "src-tauri/tauri.conf.json"), "utf8"),
);
const languages = tauriConf.bundle?.windows?.nsis?.languages ?? ["English"];

/** Mirrors VoiceSubInitInstallerLanguage in installer.nsi */
export function resolveInstallerLanguageName(langId) {
  const primary = langId & 0x3ff;
  switch (primary) {
    case 0x19:
      return "Russian";
    case 0x11:
      return "Japanese";
    case 0x12:
      return "Korean";
    case 0x04:
      return "SimpChinese";
    default:
      return "English";
  }
}

/** Representative Windows UI locales users may have (incl. unsupported -> English). */
const SYSTEM_LOCALE_MATRIX = [
  { tag: "en-US", langId: 0x0409, expect: "English" },
  { tag: "en-GB", langId: 0x0809, expect: "English" },
  { tag: "de-DE", langId: 0x0407, expect: "English" },
  { tag: "fr-FR", langId: 0x040c, expect: "English" },
  { tag: "es-ES", langId: 0x040a, expect: "English" },
  { tag: "pt-BR", langId: 0x0416, expect: "English" },
  { tag: "it-IT", langId: 0x0410, expect: "English" },
  { tag: "ru-RU", langId: 0x0419, expect: "Russian" },
  { tag: "ja-JP", langId: 0x0411, expect: "Japanese" },
  { tag: "ko-KR", langId: 0x0412, expect: "Korean" },
  { tag: "zh-CN", langId: 0x0804, expect: "SimpChinese" },
  { tag: "zh-TW", langId: 0x0404, expect: "SimpChinese" },
  { tag: "uk-UA", langId: 0x0422, expect: "English" },
];

function resolveTauriLangDir() {
  const home = process.env.USERPROFILE || process.env.HOME || "";
  const indexDir = path.join(
    home,
    ".cargo/registry/src/index.crates.io-1949cf8c6b5b557f",
  );
  const bundler = fs
    .readdirSync(indexDir)
    .filter((name) => name.startsWith("tauri-bundler-"))
    .sort()
    .at(-1);
  if (!bundler) throw new Error("tauri-bundler crate not found in cargo registry");
  return path.join(indexDir, bundler, "src/bundle/windows/nsis/languages");
}

const langDir = resolveTauriLangDir();

const LANG_FILE = {
  English: "English.nsh",
  Russian: "Russian.nsh",
  Japanese: "Japanese.nsh",
  Korean: "Korean.nsh",
  SimpChinese: "SimpChinese.nsh",
};

const LANG_CONST = {
  English: "LANG_ENGLISH",
  Russian: "LANG_RUSSIAN",
  Japanese: "LANG_JAPANESE",
  Korean: "LANG_KOREAN",
  SimpChinese: "LANG_SIMPCHINESE",
};

const installerSources = [
  path.join(root, "src-tauri/windows/installer.nsi"),
  path.join(root, "src-tauri/windows/hooks.nsh"),
];

const IGNORE = new Set([
  "RTL",
  "LANGUAGE",
  // NSIS built-in language file strings (resolved via MUI_LANGUAGE, not Tauri *.nsh)
  "UninstallingText",
  "UninstallingSubText",
  "NameDA",
  "MUI_TEXT_FINISH_RUN",
]);

/** Strings that must be non-empty on every UI surface (install + uninstall). */
const CRITICAL_UI_STRINGS = [
  "addOrReinstall",
  "uninstallApp",
  "createDesktop",
  "deleteAppData",
  "alreadyInstalledLong",
  "chooseMaintenanceOption",
  "uninstallBeforeInstalling",
  "dontUninstall",
  "voicesubWebView2Missing",
];

function extractCustomLangRefs(text) {
  const refs = new Set();
  for (const match of text.matchAll(/\$\(([A-Za-z_][A-Za-z0-9_]*)\)/g)) {
    const name = match[1];
    if (!IGNORE.has(name)) refs.add(name);
  }
  return refs;
}

function parseLangStrings(nshText) {
  const map = new Map();
  for (const match of nshText.matchAll(
    /LangString\s+(\w+)\s+\$\{LANG_\w+\}\s+"([^"]*)"/g,
  )) {
    const [, name, value] = match;
    map.set(name, value);
  }
  return map;
}

function loadLanguageTables() {
  const tables = new Map();
  for (const language of languages) {
    const fileName = LANG_FILE[language];
    if (!fileName) throw new Error(`unknown language id: ${language}`);
    const nshPath = path.join(langDir, fileName);
    if (!fs.existsSync(nshPath)) throw new Error(`missing ${nshPath}`);
    tables.set(language, parseLangStrings(fs.readFileSync(nshPath, "utf8")));
  }
  return tables;
}

function parseInlineVoiceSubStrings(installerNsi) {
  const map = new Map();
  for (const language of languages) {
    const constName = LANG_CONST[language];
    const pattern = new RegExp(
      `LangString\\s+voicesubWebView2Missing\\s+\\$\\{${constName}\\}\\s+"([^"]+)"`,
    );
    const match = installerNsi.match(pattern);
    map.set(language, match?.[1] ?? "");
  }
  return map;
}

const requiredRefs = new Set();
for (const file of installerSources) {
  const text = fs.readFileSync(file, "utf8");
  for (const ref of extractCustomLangRefs(text)) requiredRefs.add(ref);
}

const installerNsi = fs.readFileSync(
  path.join(root, "src-tauri/windows/installer.nsi"),
  "utf8",
);
const inlineVoiceSub = parseInlineVoiceSubStrings(installerNsi);
const languageTables = loadLanguageTables();
const errors = [];

for (const language of languages) {
  if (!languageTables.has(language)) {
    errors.push(`${language}: language table not loaded`);
  }
}

for (const language of languages) {
  const strings = languageTables.get(language);
  const merged = new Map(strings);
  if (inlineVoiceSub.has(language)) {
    merged.set("voicesubWebView2Missing", inlineVoiceSub.get(language));
  }

  for (const ref of requiredRefs) {
    const value = merged.get(ref);
    if (value === undefined) {
      errors.push(`${language}: missing LangString ${ref}`);
    } else if (value.trim() === "") {
      errors.push(`${language}: empty LangString ${ref}`);
    }
  }

  for (const ref of CRITICAL_UI_STRINGS) {
    const value = merged.get(ref);
    if (value === undefined || value.trim() === "") {
      errors.push(`${language}: critical UI string missing/empty: ${ref}`);
    }
  }
}

for (const locale of SYSTEM_LOCALE_MATRIX) {
  const resolved = resolveInstallerLanguageName(locale.langId);
  if (resolved !== locale.expect) {
    errors.push(
      `locale ${locale.tag} (0x${locale.langId.toString(16)}): expected ${locale.expect}, got ${resolved}`,
    );
    continue;
  }
  if (!languages.includes(resolved)) {
    errors.push(`locale ${locale.tag}: resolved language ${resolved} not bundled`);
    continue;
  }

  const strings = new Map(languageTables.get(resolved));
  if (inlineVoiceSub.has(resolved)) {
    strings.set("voicesubWebView2Missing", inlineVoiceSub.get(resolved));
  }
  for (const ref of CRITICAL_UI_STRINGS) {
    const value = strings.get(ref);
    if (value === undefined || value.trim() === "") {
      errors.push(
        `locale ${locale.tag} -> ${resolved}: critical string missing/empty: ${ref}`,
      );
    }
  }
}

const codeLines = installerNsi
  .split(/\r?\n/)
  .filter((line) => !line.trim().startsWith(";"))
  .join("\n");

if (/System::Call[^\n]*\$\([A-Za-z_]/.test(codeLines)) {
  errors.push(
    "installer.nsi uses $(LangString) inside System::Call — use StrCpy + t rN",
  );
}

if (/!insertmacro MUI_UNGETLANGUAGE/.test(codeLines)) {
  errors.push(
    "un.onInit must not use MUI_UNGETLANGUAGE — it sets $LANGUAGE to registry name string, not LANG_* id",
  );
}

if (!/SendMessage \$DeleteAppDataCheckbox \$\{WM_SETTEXT\} 0 "STR:\$VoiceSubDeleteAppDataLabel"/.test(codeLines)) {
  errors.push(
    "uninstall checkbox must use SendMessage WM_SETTEXT STR:$VoiceSubDeleteAppDataLabel (same as finish page)",
  );
}

if (/MUI_FINISHPAGE_SHOWREADME_TEXT\s+"\$\(createDesktop\)"/.test(installerNsi)) {
  errors.push(
    "MUI_FINISHPAGE_SHOWREADME_TEXT must not use $(createDesktop) in !define — use VoiceSubFinishPageShow",
  );
}

if (!/Function VoiceSubFinishPageShow/.test(installerNsi)) {
  errors.push("missing VoiceSubFinishPageShow to set finish-page checkbox labels at runtime");
}

if (!/Call VoiceSubInitInstallerLanguage/.test(installerNsi)) {
  errors.push("installer .onInit must call VoiceSubInitInstallerLanguage");
}

if (!/Call un\.VoiceSubInitInstallerLanguage/.test(installerNsi)) {
  errors.push("uninstaller un.onInit must always call un.VoiceSubInitInstallerLanguage");
}

if (
  !/Function VoiceSubInitInstallerLanguage/.test(installerNsi) &&
  !/VoiceSubInitInstallerLanguageFn/.test(installerNsi)
) {
  errors.push("missing VoiceSubInitInstallerLanguage function/macro");
}

if (!/GetUserDefaultUILanguage/.test(installerNsi)) {
  errors.push("missing GetUserDefaultUILanguage OS locale detection");
}

for (const language of languages) {
  if (!LANG_FILE[language]) {
    errors.push(`tauri.conf language not mapped to NSIS file: ${language}`);
  }
}

if (errors.length) {
  console.error("NSIS i18n validation failed:\n");
  for (const err of errors) console.error(`  - ${err}`);
  process.exit(1);
}

console.log("NSIS i18n strict audit passed");
console.log(`  bundled languages: ${languages.join(", ")}`);
console.log(
  `  custom LangString refs: ${requiredRefs.size} (all non-empty per language)`,
);
console.log(
  `  system locale matrix: ${SYSTEM_LOCALE_MATRIX.length} cases (incl. de-DE -> English fallback)`,
);
console.log("  critical UI strings verified on every resolved locale:");
for (const ref of CRITICAL_UI_STRINGS) {
  console.log(`    - ${ref}`);
}
