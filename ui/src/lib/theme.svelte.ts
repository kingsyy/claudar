// Theme management: "system" follows the OS, "light"/"dark" force a mode.
// The resolved mode is applied as a `.dark` class on <html> so all CSS tokens
// in app.css switch consistently, and `resolved` is exported for the few
// components (charts/heatmap) that must branch their colors in JS.

export type ThemePref = "system" | "light" | "dark" | "super-dark";
export type ThemeResolved = "light" | "dark" | "super-dark";

const STORAGE_KEY = "claudar-theme";

export function normalizeThemePref(value: unknown): ThemePref {
  return value === "light" || value === "dark" || value === "super-dark" || value === "system"
    ? value
    : "system";
}

export function resolveThemePreference(pref: ThemePref, systemDark: boolean): ThemeResolved {
  if (pref === "super-dark") return "super-dark";
  if (pref === "dark" || (pref === "system" && systemDark)) return "dark";
  return "light";
}

function readStored(): ThemePref {
  try {
    return normalizeThemePref(localStorage.getItem(STORAGE_KEY));
  } catch {
    // localStorage unavailable (private mode, etc.) — fall back to system.
    return "system";
  }
}

function systemPrefersDark(): boolean {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false;
}

// Reactive runes-backed state. `preference` is what the user picked;
// `resolved` is the concrete mode currently in effect.
const state = $state<{ preference: ThemePref; resolved: ThemeResolved }>({
  preference: "system",
  resolved: "light",
});

function applyResolved() {
  state.resolved = resolveThemePreference(state.preference, systemPrefersDark());
  const dark = state.resolved === "dark" || state.resolved === "super-dark";
  const superDark = state.resolved === "super-dark";
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.classList.toggle("super-dark", superDark);
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
  state.preference = normalizeThemePref(pref);
  try {
    localStorage.setItem(STORAGE_KEY, state.preference);
  } catch {
    // ignore persistence failures
  }
  applyResolved();
}

/** Cycle through light → dark → system for a single toggle button. */
export function cycleTheme() {
  const order: ThemePref[] = ["light", "dark", "super-dark", "system"];
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
    return state.resolved === "dark" || state.resolved === "super-dark";
  },
};
