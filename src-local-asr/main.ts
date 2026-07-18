import { mount } from "svelte";
import App from "./App.svelte";
import { bootstrapLocalAsrFromSettings } from "./lib/app-settings";
import "./styles/local-asr-module.css";

void bootstrapLocalAsrFromSettings().then((cleanup) => {
  mount(App, { target: document.getElementById("app")! });
  window.addEventListener("pagehide", cleanup);
  window.addEventListener("beforeunload", cleanup);
});
