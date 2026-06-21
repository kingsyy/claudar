// Theme management: "system" follows the OS, "light"/"dark" force a mode.
// The resolved mode is applied as a `.dark` class on <html> so all CSS tokens
// in app.css switch consistently, and `resolved` is exported for the few
// components (charts/heatmap) that must branch their colors in JS.

export type ThemePref = "system" | "light" | "dark";

const STORAGE_KEY = "claudar-theme";

function readStored(): ThemePref {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    // localStorage unavailable (private mode, etc.) — fall back to system.
  }
  return "system";
}

function systemPrefersDark(): boolean {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false;
}

// Reactive runes-backed state. `preference` is what the user picked;
// `resolved` is the concrete mode currently in effect.
const state = $state<{ preference: ThemePref; resolved: "light" | "dark" }>({
  preference: "system",
  resolved: "light",
});

function applyResolved() {
  const dark = state.preference === "dark" || (state.preference === "system" && systemPrefersDark());
  state.resolved = dark ? "dark" : "light";
  document.documentElement.classList.toggle("dark", dark);
  // Keep native controls (scrollbars, date pickers, form fields) in sync.
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
}

/** Call once at startup, before the app mounts, to avoid a flash. */
export function initTheme() {
  state.preference = readStored();
  applyResolved();
  // Keep "system" in sync with live OS changes.
  window.matchMedia?.("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (state.preference === "system") applyResolved();
  });
}

export function setTheme(pref: ThemePref) {
  state.preference = pref;
  try {
    localStorage.setItem(STORAGE_KEY, pref);
  } catch {
    // ignore persistence failures
  }
  applyResolved();
}

/** Cycle through light → dark → system for a single toggle button. */
export function cycleTheme() {
  const order: ThemePref[] = ["light", "dark", "system"];
  const next = order[(order.indexOf(state.preference) + 1) % order.length];
  setTheme(next);
}

export const theme = {
  get preference() {
    return state.preference;
  },
  get resolved() {
    return state.resolved;
  },
  get isDark() {
    return state.resolved === "dark";
  },
};
