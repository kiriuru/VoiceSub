import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const targets = [path.join(root, "frontend", "js", "i18n")];

for (const dir of targets) {
  const localesDir = path.join(dir, "locales");
  const locales = fs.readdirSync(localesDir).filter((f) => f.endsWith(".js")).map((f) => f.replace(/\.js$/, ""));
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
  const outPath = path.join(dir, "locales-bundle.js");
  fs.writeFileSync(outPath, out);
  console.log(`wrote ${outPath}`);
}
