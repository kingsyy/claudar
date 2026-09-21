import { normalizeProvider } from "$lib/provider";

const MIN_PROJECTION_TIME_PCT = 5;

/** An explicit per-channel color that overrides the zone's auto-derived
 *  shade — a literal hex, not a hue, so it does not adapt across light/dark/
 *  super-dark the way the derived shades do. Absent channels keep deriving
 *  from `hue`/`saturation` as before. */
export type PaceZoneOverrides = {
  bg?: string;
  border?: string;
  text?: string;
};

/**
 * A user-editable band of the pace bar. `upTo` is this zone's exclusive upper
 * bound (on the projected-peak percentage); `null` only on the last zone,
 * which is open-ended above. Zones are a sorted, contiguous, non-overlapping
 * list by construction — there's no separate min, since it's always the
 * previous zone's `upTo` (or 0 for the first).
 */
export type PaceZone = {
  id: string;
  upTo: number | null;
  hue: number;
  saturation: number;
  label: string;
  overrides?: PaceZoneOverrides;
};

/** The zone whose range contains `peak`, walking zones in order since each
 *  one's range ends where the next begins. */
export function matchZone(peak: number, zones: PaceZone[]): PaceZone {
  return zones.find((z) => z.upTo == null || peak < z.upTo) ?? zones[zones.length - 1];
}

/**
 * Estimate usage at reset. Backend predictions win when available; otherwise
 * extrapolate after enough of the window has elapsed to avoid startup noise.
 */
export function projectedPeak(
  pct: number,
  timePct: number,
  predicted: number | null,
): number {
  if (predicted != null) return predicted;
  if (timePct < MIN_PROJECTION_TIME_PCT) return pct;
  return (pct / timePct) * 100;
}

/** Format the reset instant exactly enough to display and copy. */
export function formatResetTimestamp(
  iso: string | null | undefined,
  locales?: Intl.LocalesArgument,
  timeZone?: string,
): string {
  if (!iso) return "—";

  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";

  return new Intl.DateTimeFormat(locales, {
    dateStyle: "medium",
    timeStyle: "long",
    timeZone,
  }).format(date);
}

/**
 * Length of a rolling window in ms, from the seconds the provider reported.
 * Falls back for payloads from a backend that predates the field.
 */
export function windowMs(seconds: number | null | undefined, fallbackMs: number): number {
  return seconds != null && seconds > 0 ? seconds * 1000 : fallbackMs;
}

/**
 * Name a rolling window after its actual length ("5-hour", "7-day") rather than
 * assuming Claude's pair, so a provider reporting a different period is labelled
 * for what it is instead of being mislabelled.
 */
export function windowLabel(ms: number): string {
  const hours = Math.round(ms / 3_600_000);
  if (hours >= 24 && hours % 24 === 0) return `${hours / 24}-day`;
  if (hours >= 1) return `${hours}-hour`;
  return `${Math.max(1, Math.round(ms / 60_000))}-minute`;
}

/**
 * Whether to draw the short-window bar. ChatGPT accounts can report only a
 * weekly window; drawing a 0% 5-hour bar for them would be inventing data.
 * Claude always reports both, including on payloads written before the field
 * existed.
 */
export function hasShortWindow(
  provider: string | undefined,
  windowSeconds: number | null | undefined,
): boolean {
  return normalizeProvider(provider) === "claude-web" || windowSeconds != null;
}
