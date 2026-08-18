/** Keyboard movement for an ARIA tablist, kept pure so the wrapping is testable. */
export function nextTabIndex(key: string, index: number, count: number): number | null {
  if (count <= 0) return null;

  const target: Record<string, number> = {
    ArrowRight: index + 1,
    ArrowLeft: index - 1,
    Home: 0,
    End: count - 1,
  };

  const next = target[key];
  if (next === undefined) return null;

  // Wrap in both directions — ArrowLeft on the first tab lands on the last.
  return ((next % count) + count) % count;
}
