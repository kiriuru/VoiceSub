import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function buildBundle(localesDir, outPath) {
  const locales = fs
    .readdirSync(localesDir)
    .filter((f) => f.endsWith(".js"))
    .map((f) => f.replace(/\.js$/, ""));
  let out = "(function () {\n  window.__SST_I18N_LOCALES = window.__SST_I18N_LOCALES || {};\n";
  for (const locale of locales) {
    const code = fs.readFileSync(path.join(localesDir, `${locale}.js`), "utf8");
    const sandbox = { window: { __SST_I18N_LOCALES: {} } };
    vm.runInNewContext(code, sandbox);
    const messages = sandbox.window.__SST_I18N_LOCALES[locale];
    if (!messages) continue;
    out += `  window.__SST_I18N_LOCALES.${locale} = ${JSON.stringify(messages, null, 2)};\n`;
  }
  out += "})();\n";
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, out);
  console.log(`wrote ${outPath}`);
}

const sourceLocalesDir = path.join(root, "scripts", "i18n-source", "locales");
const bundleTargets = [
  path.join(root, "scripts", "i18n-source", "locales-bundle.js"),
  path.join(root, "bin", "overlay", "shared", "js", "i18n", "locales-bundle.js"),
];

for (const outPath of bundleTargets) {
  buildBundle(sourceLocalesDir, outPath);
}

const dynamicSource = path.join(root, "scripts", "i18n-source", "dynamic-locales.js");
const dynamicTarget = path.join(root, "bin", "overlay", "shared", "js", "i18n", "dynamic-locales.js");
fs.mkdirSync(path.dirname(dynamicTarget), { recursive: true });
fs.copyFileSync(dynamicSource, dynamicTarget);
console.log(`copied ${dynamicSource} -> ${dynamicTarget}`);
