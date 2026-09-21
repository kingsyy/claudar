import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";
import { initTheme } from "./lib/theme.svelte";
import { initVisuals } from "./lib/visuals.svelte";
import { initPaceZones } from "./lib/paceZones.svelte";

initTheme();
initVisuals();
initPaceZones();

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
