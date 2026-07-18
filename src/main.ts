import App from "./App.svelte";
import { mount } from "svelte";
import { markDesktopShell } from "./lib/shell-platform";
import { applyUiColorSchemeToDocument, readStoredUiTheme } from "./lib/ui-theme-css";
import "./lib/styles/global.css";
import "./lib/styles/compact-layout.css";

// Paint last-known theme before settings HTTP returns (avoids dark→light flash).
const storedTheme = readStoredUiTheme();
if (storedTheme) {
  applyUiColorSchemeToDocument(storedTheme);
}

markDesktopShell();

const target = document.getElementById("app");
if (target) {
  mount(App, { target });
}
