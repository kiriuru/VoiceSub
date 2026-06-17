import App from "./App.svelte";
import { mount } from "svelte";
import { markDesktopShell } from "./lib/shell-platform";
import "./lib/styles/global.css";
import "./lib/styles/compact-layout.css";

markDesktopShell();

const target = document.getElementById("app");
if (target) {
  mount(App, { target });
}
