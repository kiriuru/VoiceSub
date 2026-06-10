import App from "./App.svelte";
import { mount } from "svelte";
import "./lib/styles/global.css";
import "./lib/styles/compact-layout.css";

const target = document.getElementById("app");
if (target) {
  mount(App, { target });
}
