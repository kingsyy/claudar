const MIN_PROJECTION_TIME_PCT = 5;

/** A projected overage of up to this amount is considered acceptable pace. */
export const ACCEPTABLE_OVERAGE_PCT = 10;
/** Above this projection, the pace needs urgent attention rather than a warning. */
export const CRITICAL_PROJECTED_PEAK_PCT = 125;

export type PaceVerdict = "ok" | "warn" | "crit";

/**
 * Give pace a clear, three-level meaning: green through a small (<=10%) buffer,
 * amber for a moderate overage, and red only for a large overage or a cap hit.
 */
export function paceVerdict(pct: number, peak: number): PaceVerdict {
  if (pct >= 99 || peak >= CRITICAL_PROJECTED_PEAK_PCT) return "crit";
  if (peak > 100 + ACCEPTABLE_OVERAGE_PCT) return "warn";
  return "ok";
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
