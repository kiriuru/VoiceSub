import { mount } from "svelte";
import App from "./App.svelte";
import { bootstrapTtsFromSettings } from "./lib/app-settings";
import "./styles/tts-module.css";

void bootstrapTtsFromSettings().finally(() => {
  mount(App, { target: document.getElementById("app")! });
});
