import { mount } from "svelte";
import App from "./App.svelte";
import { bootstrapLocalAsrFromSettings } from "./lib/app-settings";
import "./styles/local-asr-module.css";

void bootstrapLocalAsrFromSettings().finally(() => {
  mount(App, { target: document.getElementById("app")! });
});
