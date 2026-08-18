export type StatusPalette = "standard" | "colorblind";

/** Scales the root font size; every size in the app is in `rem`, so this scales all of it. */
export type TextSize = "small" | "default" | "large" | "larger";

export const TEXT_SIZES: TextSize[] = ["small", "default", "large", "larger"];

export type VisualPreferences = {
  reduceMotion: boolean;
  highContrast: boolean;
  statusPalette: StatusPalette;
  textSize: TextSize;
};

const STORAGE_KEYS = {
  reduceMotion: "claudar-reduce-motion",
  highContrast: "claudar-high-contrast",
  statusPalette: "claudar-status-palette",
  textSize: "claudar-text-size",
} as const;

function readBoolean(key: string): boolean {
  try {
    return localStorage.getItem(key) === "true";
  } catch {
    return false;
  }
}

function readPalette(): StatusPalette {
  try {
    return localStorage.getItem(STORAGE_KEYS.statusPalette) === "colorblind"
      ? "colorblind"
      : "standard";
  } catch {
    return "standard";
  }
}

function readTextSize(): TextSize {
  try {
    const stored = localStorage.getItem(STORAGE_KEYS.textSize);
    return TEXT_SIZES.includes(stored as TextSize) ? (stored as TextSize) : "default";
  } catch {
    return "default";
  }
}

/** Classes for explicit user selections, kept pure for reliable testing. */
export function visualClasses(preferences: VisualPreferences): string[] {
  const classes: string[] = [];
  if (preferences.reduceMotion) classes.push("reduce-motion");
  if (preferences.highContrast) classes.push("high-contrast");
  if (preferences.statusPalette === "colorblind") classes.push("status-colorblind");
  // "default" needs no class — it's the stylesheet's own root size.
  if (preferences.textSize !== "default") classes.push(`text-${preferences.textSize}`);
  return classes;
}

const state = $state<VisualPreferences>({
  reduceMotion: false,
  highContrast: false,
  statusPalette: "standard",
  textSize: "default",
});

function systemPrefersReducedMotion(): boolean {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

function applyVisuals() {
  const root = document.documentElement;
  root.classList.toggle("reduce-motion", state.reduceMotion || systemPrefersReducedMotion());
  root.classList.toggle("high-contrast", state.highContrast);
  root.classList.toggle("status-colorblind", state.statusPalette === "colorblind");
  for (const size of TEXT_SIZES) {
    root.classList.toggle(`text-${size}`, size !== "default" && state.textSize === size);
  }
}

function save(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // The preference remains active for this session if storage is unavailable.
  }
}

/** Call once during app startup to restore visual accessibility preferences. */
export function initVisuals() {
  state.reduceMotion = readBoolean(STORAGE_KEYS.reduceMotion);
  state.highContrast = readBoolean(STORAGE_KEYS.highContrast);
  state.statusPalette = readPalette();
  state.textSize = readTextSize();
  applyVisuals();

  window.matchMedia?.("(prefers-reduced-motion: reduce)").addEventListener("change", () => {
    applyVisuals();
  });
}

export function setReduceMotion(enabled: boolean) {
  state.reduceMotion = enabled;
  save(STORAGE_KEYS.reduceMotion, String(enabled));
  applyVisuals();
}

export function setHighContrast(enabled: boolean) {
  state.highContrast = enabled;
  save(STORAGE_KEYS.highContrast, String(enabled));
  applyVisuals();
}

export function setStatusPalette(palette: StatusPalette) {
  state.statusPalette = palette;
  save(STORAGE_KEYS.statusPalette, palette);
  applyVisuals();
}

export function setTextSize(size: TextSize) {
  state.textSize = size;
  save(STORAGE_KEYS.textSize, size);
  applyVisuals();
}

export const visuals = {
  get reduceMotion() {
    return state.reduceMotion;
  },
  get highContrast() {
    return state.highContrast;
  },
  get statusPalette() {
    return state.statusPalette;
  },
  get textSize() {
    return state.textSize;
  },
};
