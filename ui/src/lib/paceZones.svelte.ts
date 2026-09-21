import { matchZone, type PaceZone, type PaceZoneOverrides } from "./dashboard";

export type PaceWindow = "five_hour" | "seven_day";
export type PaceZoneMap = Record<PaceWindow, PaceZone[]>;
export type OverrideChannel = keyof PaceZoneOverrides;

const STORAGE_KEY = "claudar-pace-zones";
const MAX_ZONES = 8;
const MIN_ZONE_GAP_PCT = 1;
const UNDO_SECONDS = 10;
/** WCAG AA's minimum for UI components/graphics (not the stricter 4.5:1 for
 *  body text) — advisory threshold for the low-contrast warning below. */
const MIN_CONTRAST_RATIO = 3;
/** Representative lightness for each channel, matching the ladder in
 *  app.css's light-theme `.zone-fill`/`.zone-text` rules — used only to seed
 *  the "Advanced" color pickers with the zone's current derived look. */
const DERIVED_LIGHTNESS: Record<OverrideChannel, number> = { bg: 95, border: 65, text: 28 };

function zoneId(): string {
  return typeof crypto !== "undefined" && crypto.randomUUID
    ? crypto.randomUUID()
    : `zone-${Math.random().toString(36).slice(2)}`;
}

/** Same breakpoints as the fixed tiers this replaces: on/under pace, a small
 *  (<=10%) projected overage (the goldilocks sweet spot — a limit that resets
 *  on a timer is wasted if it's never used up), a moderate overage, and a
 *  large one or a capped limit. */
export function defaultZones(): PaceZone[] {
  return [
    { id: zoneId(), upTo: 100, hue: 142, saturation: 70, label: "Under pace" },
    { id: zoneId(), upTo: 110, hue: 175, saturation: 70, label: "Sweet spot" },
    { id: zoneId(), upTo: 125, hue: 38, saturation: 90, label: "Over pace" },
    { id: zoneId(), upTo: null, hue: 0, saturation: 80, label: "Critical" },
  ];
}

/** Blue / bluish-green / orange / violet — spaced to stay distinguishable for
 *  red-green colour vision deficiencies, matching the hues already validated
 *  in the app's `status-colorblind` palette. */
export function colorblindZones(): PaceZone[] {
  return [
    { id: zoneId(), upTo: 100, hue: 203, saturation: 100, label: "Under pace" },
    { id: zoneId(), upTo: 110, hue: 160, saturation: 70, label: "Sweet spot" },
    { id: zoneId(), upTo: 125, hue: 36, saturation: 100, label: "Over pace" },
    { id: zoneId(), upTo: null, hue: 294, saturation: 75, label: "Critical" },
  ];
}

/**
 * Split the zone `id` into two at `splitAt`, copying its colour into the new,
 * lower zone. Refuses (returns the input unchanged) if `splitAt` doesn't fall
 * strictly inside that zone's own range, if it would leave either side
 * thinner than `MIN_ZONE_GAP_PCT`, or if the zone count is already at the cap.
 */
export function splitZoneAt(zones: PaceZone[], id: string, splitAt: number): PaceZone[] {
  if (zones.length >= MAX_ZONES) return zones;
  const i = zones.findIndex((z) => z.id === id);
  if (i === -1) return zones;
  const lower = i === 0 ? 0 : (zones[i - 1].upTo as number);
  const upper = zones[i].upTo;
  if (splitAt < lower + MIN_ZONE_GAP_PCT) return zones;
  if (upper != null && splitAt > upper - MIN_ZONE_GAP_PCT) return zones;
  const zone = zones[i];
  const next = [...zones];
  next.splice(i, 0, {
    id: zoneId(),
    upTo: splitAt,
    hue: zone.hue,
    saturation: zone.saturation,
    label: "New zone",
  });
  return next;
}

/**
 * Remove the zone `id` by merging it into its preceding zone (which absorbs
 * its upper bound). The first zone has no preceding zone, so removing it just
 * drops it — the next zone then implicitly starts at 0%. Refuses to drop the
 * last remaining zone.
 */
export function mergeZone(zones: PaceZone[], id: string): PaceZone[] {
  if (zones.length <= 1) return zones;
  const i = zones.findIndex((z) => z.id === id);
  if (i === -1) return zones;
  const next = [...zones];
  if (i === 0) {
    next.splice(0, 1);
  } else {
    next[i - 1] = { ...next[i - 1], upTo: next[i].upTo };
    next.splice(i, 1);
  }
  return next;
}

export function setZoneColor(zones: PaceZone[], id: string, hue: number, saturation: number): PaceZone[] {
  return zones.map((z) => (z.id === id ? { ...z, hue, saturation } : z));
}

/**
 * Move the boundary between zone `id` and the zone after it to `upTo`. A no-op
 * on the last zone, which has no boundary of its own to move. Refuses a value
 * that would leave either neighbouring zone thinner than the minimum gap.
 */
export function moveBoundary(zones: PaceZone[], id: string, upTo: number): PaceZone[] {
  const i = zones.findIndex((z) => z.id === id);
  if (i === -1 || zones[i].upTo == null) return zones;
  const lower = i === 0 ? 0 : (zones[i - 1].upTo as number);
  const upper = zones[i + 1]?.upTo ?? null;
  if (upTo < lower + MIN_ZONE_GAP_PCT) return zones;
  if (upper != null && upTo > upper - MIN_ZONE_GAP_PCT) return zones;
  return zones.map((z, idx) => (idx === i ? { ...z, upTo } : z));
}

/** Round-trips through sRGB, discarding the lightness the user picked — the
 *  zone's rendered lightness always comes from the CSS ladder in app.css, so
 *  only the hue and saturation the picker chose are kept. */
export function hexToHsl(hex: string): { hue: number; saturation: number } {
  const r = parseInt(hex.slice(1, 3), 16) / 255;
  const g = parseInt(hex.slice(3, 5), 16) / 255;
  const b = parseInt(hex.slice(5, 7), 16) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return { hue: 0, saturation: 0 };
  const saturation = d / (1 - Math.abs(2 * l - 1));
  let hue: number;
  if (max === r) hue = ((g - b) / d) % 6;
  else if (max === g) hue = (b - r) / d + 2;
  else hue = (r - g) / d + 4;
  hue *= 60;
  if (hue < 0) hue += 360;
  return { hue: Math.round(hue), saturation: Math.round(saturation * 100) };
}

/** Renders a zone's hue/saturation as a hex swatch at a fixed representative
 *  lightness, purely for the `<input type="color">` preview — the bar itself
 *  never uses this lightness. */
export function hslToHex(hue: number, saturation: number, lightness: number): string {
  const s = saturation / 100;
  const l = lightness / 100;
  const k = (n: number) => (n + hue / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  const toHex = (x: number) => Math.round(255 * x).toString(16).padStart(2, "0");
  return `#${toHex(f(0))}${toHex(f(8))}${toHex(f(4))}`;
}

export function setZoneLabel(zones: PaceZone[], id: string, label: string): PaceZone[] {
  return zones.map((z) => (z.id === id ? { ...z, label } : z));
}

/** The `--zone-h`/`--zone-s` (plus any literal `--zone-*-override`) inline
 *  style every zone-coloured element needs — shared by Dashboard's real bars
 *  and Settings' preview strip so the two can never drift out of sync. */
export function zoneStyle(zone: PaceZone, extra = ""): string {
  const parts = [extra, `--zone-h: ${zone.hue}`, `--zone-s: ${zone.saturation}%`];
  if (zone.overrides?.bg) parts.push(`--zone-bg-override: ${zone.overrides.bg}`);
  if (zone.overrides?.border) parts.push(`--zone-border-override: ${zone.overrides.border}`);
  if (zone.overrides?.text) parts.push(`--zone-text-override: ${zone.overrides.text}`);
  return parts.filter(Boolean).join("; ");
}

/** Set one literal-color channel override on a zone, replacing whatever the
 *  hue/saturation ladder would otherwise derive for it. */
export function setZoneOverride(
  zones: PaceZone[],
  id: string,
  channel: OverrideChannel,
  hex: string,
): PaceZone[] {
  return zones.map((z) =>
    z.id === id ? { ...z, overrides: { ...z.overrides, [channel]: hex } } : z,
  );
}

/** Drop all overrides on a zone, returning it to the fully auto-derived look. */
export function clearOverrides(zones: PaceZone[], id: string): PaceZone[] {
  return zones.map((z) => {
    if (z.id !== id) return z;
    const { overrides: _overrides, ...rest } = z;
    return rest;
  });
}

/** The hex a zone's channel would render as today, absent an override — used
 *  only to seed the "Advanced" color pickers so opening the panel without
 *  touching anything shows the same color already on screen. */
export function derivedZoneHex(zone: PaceZone, channel: OverrideChannel): string {
  return hslToHex(zone.hue, zone.saturation, DERIVED_LIGHTNESS[channel]);
}

function srgbToLinear(channel255: number): number {
  const c = channel255 / 255;
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

function relativeLuminance(hex: string): number {
  const r = srgbToLinear(parseInt(hex.slice(1, 3), 16));
  const g = srgbToLinear(parseInt(hex.slice(3, 5), 16));
  const b = srgbToLinear(parseInt(hex.slice(5, 7), 16));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio between two hex colors, from 1 (identical) to 21
 *  (black on white). */
export function contrastRatio(hexA: string, hexB: string): number {
  const a = relativeLuminance(hexA);
  const b = relativeLuminance(hexB);
  const lighter = Math.max(a, b);
  const darker = Math.min(a, b);
  return (lighter + 0.05) / (darker + 0.05);
}

/** Whether a zone's own text/bg override pair reads as low-contrast — `false`
 *  when either channel isn't overridden, since there's nothing user-chosen to
 *  warn about (the derived pair is always readable by construction). */
export function hasLowContrastOverride(zone: PaceZone): boolean {
  const { text, bg } = zone.overrides ?? {};
  if (!text || !bg) return false;
  return contrastRatio(text, bg) < MIN_CONTRAST_RATIO;
}

export { matchZone };
export type { PaceZone };

function readZones(): PaceZoneMap {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { five_hour: defaultZones(), seven_day: defaultZones() };
    const parsed = JSON.parse(raw) as Partial<PaceZoneMap>;
    if (!parsed.five_hour?.length || !parsed.seven_day?.length) throw new Error("incomplete");
    return { five_hour: parsed.five_hour, seven_day: parsed.seven_day };
  } catch {
    return { five_hour: defaultZones(), seven_day: defaultZones() };
  }
}

function persist(zones: PaceZoneMap) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(zones));
  } catch {
    // The edit stays active for this session if storage is unavailable.
  }
}

const state = $state<PaceZoneMap>({ five_hour: [], seven_day: [] });
let secondsLeft = $state(0);

// Pre-edit snapshot for the confirm/revert countdown, and the interval
// driving it. Not `$state`: neither needs to trigger a re-render on its own,
// only the derived `secondsLeft` and `state` fields they update do.
let snapshot: PaceZoneMap | null = null;
let timer: ReturnType<typeof setInterval> | undefined;

// Letting the countdown run out only dismisses the "Undo" option — it must
// NOT discard the edit. (An earlier version reverted-and-persisted on
// timeout, which meant switching to the Dashboard to check the result — a
// completely normal thing to do — silently threw the change away before you
// got back to Settings. Auto-save with an opt-in undo is the only version of
// this that isn't fragile: the edit is safe the instant it's made, full stop.)
function tick() {
  secondsLeft -= 1;
  if (secondsLeft <= 0) dismissUndo();
}

function stopTimer() {
  if (timer) clearInterval(timer);
  timer = undefined;
}

function beginEdit() {
  if (snapshot === null) {
    snapshot = { five_hour: [...state.five_hour], seven_day: [...state.seven_day] };
  }
  secondsLeft = UNDO_SECONDS;
  stopTimer();
  timer = setInterval(tick, 1000);
}

function cancelPendingUndo() {
  stopTimer();
  snapshot = null;
}

function applyEdit(window: PaceWindow, next: PaceZone[]) {
  beginEdit();
  state[window] = next;
  persist({ five_hour: state.five_hour, seven_day: state.seven_day });
}

/** Call once during app startup to restore saved zones. */
export function initPaceZones() {
  const loaded = readZones();
  state.five_hour = loaded.five_hour;
  state.seven_day = loaded.seven_day;
}

export function splitZone(window: PaceWindow, id: string, splitAt: number) {
  applyEdit(window, splitZoneAt(state[window], id, splitAt));
}

export function removeZone(window: PaceWindow, id: string) {
  applyEdit(window, mergeZone(state[window], id));
}

export function moveZoneBoundary(window: PaceWindow, id: string, upTo: number) {
  applyEdit(window, moveBoundary(state[window], id, upTo));
}

export function recolorZone(window: PaceWindow, id: string, hue: number, saturation: number) {
  applyEdit(window, setZoneColor(state[window], id, hue, saturation));
}

export function relabelZone(window: PaceWindow, id: string, label: string) {
  applyEdit(window, setZoneLabel(state[window], id, label));
}

export function overrideZoneColor(window: PaceWindow, id: string, channel: OverrideChannel, hex: string) {
  applyEdit(window, setZoneOverride(state[window], id, channel, hex));
}

export function resetZoneOverrides(window: PaceWindow, id: string) {
  applyEdit(window, clearOverrides(state[window], id));
}

/** Presets are vetted, not free-form edits — apply and persist immediately,
 *  no confirm/revert countdown. */
function applyPreset(window: PaceWindow, zones: PaceZone[]) {
  cancelPendingUndo();
  state[window] = zones;
  persist({ five_hour: state.five_hour, seven_day: state.seven_day });
}

export function resetZones(window: PaceWindow) {
  applyPreset(window, defaultZones());
}

export function useColorblindZones(window: PaceWindow) {
  applyPreset(window, colorblindZones());
}

/** Dismiss the "Undo" toast early. The edit is already saved — this only
 *  clears the option to undo it, nothing more. */
export function dismissUndo() {
  stopTimer();
  snapshot = null;
}

/** Explicitly undo every edit since the snapshot was taken, restoring and
 *  re-persisting the prior state. Only reachable via the toast's own button —
 *  it never fires on its own. */
export function undoEdit() {
  stopTimer();
  if (snapshot) {
    state.five_hour = snapshot.five_hour;
    state.seven_day = snapshot.seven_day;
    persist(snapshot);
  }
  snapshot = null;
}

export const paceZones = {
  get five_hour() {
    return state.five_hour;
  },
  get seven_day() {
    return state.seven_day;
  },
  get secondsLeft() {
    return secondsLeft;
  },
  get pendingUndo() {
    return snapshot !== null;
  },
};
