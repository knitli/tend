/**
 * Unified filter predicate for session list matching (FR-006).
 *
 * A session matches iff the lowercased query appears in either the session
 * label or the project display name. Empty/whitespace-only queries match
 * everything.
 */
export function matchesSessionFilter(
  query: string,
  sessionLabel: string,
  projectDisplayName: string,
): boolean {
  const q = query.toLowerCase().trim();
  if (!q) return true;
  return (
    sessionLabel.toLowerCase().includes(q) ||
    projectDisplayName.toLowerCase().includes(q)
  );
}
