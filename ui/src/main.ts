import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";
import { initTheme } from "./lib/theme.svelte";
import { initVisuals } from "./lib/visuals.svelte";

initTheme();
initVisuals();

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
