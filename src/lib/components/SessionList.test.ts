// T100: Vitest unit test for the unified filter predicate in SessionList.
//
// Seeds three sessions across two projects with deliberately overlapping names
// and asserts that query "alpha" matches the correct rows.

import { describe, it, expect } from 'vitest';

// The filter predicate extracted for testability.
// Mirrors the logic in SessionList.svelte's filteredSessions derived.
function matchesFilter(
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

describe('SessionList filter predicate', () => {
  // Seed data:
  // Project "alpha project" with session "beta refactor"
  // Project "beta project" with session "alpha rewrite"
  // Project "gamma project" with session "gamma init"

  const sessions = [
    { label: 'beta refactor', projectName: 'alpha project' },
    { label: 'alpha rewrite', projectName: 'beta project' },
    { label: 'gamma init', projectName: 'gamma project' },
  ];

  it('empty query matches everything', () => {
    const results = sessions.filter((s) => matchesFilter('', s.label, s.projectName));
    expect(results).toHaveLength(3);
  });

  it('"alpha" matches both sessions with alpha in label or project name', () => {
    const results = sessions.filter((s) => matchesFilter('alpha', s.label, s.projectName));
    expect(results).toHaveLength(2);
    // "beta refactor" matched via project name "alpha project"
    expect(results[0].label).toBe('beta refactor');
    // "alpha rewrite" matched via session label
    expect(results[1].label).toBe('alpha rewrite');
  });

  it('"beta" matches both sessions with beta in label or project name', () => {
    const results = sessions.filter((s) => matchesFilter('beta', s.label, s.projectName));
    expect(results).toHaveLength(2);
    expect(results[0].label).toBe('beta refactor');
    expect(results[1].label).toBe('alpha rewrite');
  });

  it('"gamma" matches only the gamma session', () => {
    const results = sessions.filter((s) => matchesFilter('gamma', s.label, s.projectName));
    expect(results).toHaveLength(1);
    expect(results[0].label).toBe('gamma init');
  });

  it('"nonexistent" matches nothing', () => {
    const results = sessions.filter((s) => matchesFilter('nonexistent', s.label, s.projectName));
    expect(results).toHaveLength(0);
  });

  it('filter is case-insensitive', () => {
    const results = sessions.filter((s) => matchesFilter('ALPHA', s.label, s.projectName));
    expect(results).toHaveLength(2);
  });

  it('whitespace-only query matches everything', () => {
    const results = sessions.filter((s) => matchesFilter('   ', s.label, s.projectName));
    expect(results).toHaveLength(3);
  });
});
