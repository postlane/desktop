// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest';
import {
  parseTimestamp,
  isActive,
  getRepoStatus,
  sortAndBucketRepos,
} from './navUtils';
import type { RepoWithStatus } from '../types';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeRepo(overrides: Partial<RepoWithStatus> = {}): RepoWithStatus {
  return {
    id: 'r1',
    name: 'Test Repo',
    path: '/path/to/repo',
    active: true,
    added_at: '2024-01-01T00:00:00Z',
    path_exists: true,
    ready_count: 0,
    failed_count: 0,
    last_post_at: null,
    provider: null,
    ...overrides,
  };
}

function daysAgo(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return d.toISOString();
}

// ---------------------------------------------------------------------------
// parseTimestamp
// ---------------------------------------------------------------------------

describe('parseTimestamp', () => {
  it('returns null for null input', () => {
    expect(parseTimestamp(null)).toBeNull();
  });

  it('returns null for undefined input', () => {
    expect(parseTimestamp(undefined)).toBeNull();
  });

  it('returns null for empty string', () => {
    expect(parseTimestamp('')).toBeNull();
  });

  it('returns null for invalid date string', () => {
    expect(parseTimestamp('not-a-date')).toBeNull();
  });

  it('returns a valid Date for ISO 8601 string', () => {
    const d = parseTimestamp('2024-06-01T10:00:00Z');
    expect(d).toBeInstanceOf(Date);
    expect(d?.toISOString()).toBe('2024-06-01T10:00:00.000Z');
  });
});

// ---------------------------------------------------------------------------
// isActive
// ---------------------------------------------------------------------------

describe('isActive', () => {
  it('returns false for null (no posts)', () => {
    expect(isActive(null)).toBe(false);
  });

  it('returns true when last activity was 1 day ago', () => {
    expect(isActive(new Date(Date.now() - 1 * 24 * 60 * 60 * 1000))).toBe(true);
  });

  it('returns true when last activity was 29 days ago', () => {
    expect(isActive(new Date(Date.now() - 29 * 24 * 60 * 60 * 1000))).toBe(true);
  });

  it('returns false when last activity was 31 days ago', () => {
    expect(isActive(new Date(Date.now() - 31 * 24 * 60 * 60 * 1000))).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// getRepoStatus
// ---------------------------------------------------------------------------

describe('getRepoStatus', () => {
  it('returns warning when path does not exist', () => {
    const status = getRepoStatus(makeRepo({ path_exists: false }));
    expect(status).toEqual({ type: 'warning' });
  });

  it('returns none when repo is inactive (deliberately disabled)', () => {
    const status = getRepoStatus(makeRepo({ active: false }));
    expect(status).toEqual({ type: 'none' });
  });

  it('returns watching when active repo has no pending posts', () => {
    const status = getRepoStatus(makeRepo({ ready_count: 0, failed_count: 0 }));
    expect(status).toEqual({ type: 'watching' });
  });

  it('returns single green when only ready posts', () => {
    const status = getRepoStatus(makeRepo({ ready_count: 3 }));
    expect(status).toEqual({ type: 'single', color: 'green' });
  });

  it('returns single red when only failed posts', () => {
    const status = getRepoStatus(makeRepo({ failed_count: 2 }));
    expect(status).toEqual({ type: 'single', color: 'red' });
  });

  it('returns stacked when both ready and failed posts exist', () => {
    const status = getRepoStatus(makeRepo({ ready_count: 1, failed_count: 1 }));
    expect(status).toEqual({ type: 'stacked' });
  });

  it('warning takes priority over active/inactive check', () => {
    // path_exists=false overrides active=false
    const status = getRepoStatus(makeRepo({ path_exists: false, active: false }));
    expect(status).toEqual({ type: 'warning' });
  });
});

// ---------------------------------------------------------------------------
// sortAndBucketRepos
// ---------------------------------------------------------------------------

describe('sortAndBucketRepos', () => {
  it('returns empty buckets for empty input', () => {
    const { active, inactive } = sortAndBucketRepos([]);
    expect(active).toHaveLength(0);
    expect(inactive).toHaveLength(0);
  });

  it('places repos with recent activity in the active bucket', () => {
    const repo = makeRepo({ last_post_at: daysAgo(5) });
    const { active, inactive } = sortAndBucketRepos([repo]);
    expect(active).toHaveLength(1);
    expect(inactive).toHaveLength(0);
  });

  it('places repos with no activity in the inactive bucket', () => {
    const repo = makeRepo({ last_post_at: null });
    const { active, inactive } = sortAndBucketRepos([repo]);
    expect(active).toHaveLength(0);
    expect(inactive).toHaveLength(1);
  });

  it('places repos with stale activity in the inactive bucket', () => {
    const repo = makeRepo({ last_post_at: daysAgo(40) });
    const { active, inactive } = sortAndBucketRepos([repo]);
    expect(active).toHaveLength(0);
    expect(inactive).toHaveLength(1);
  });

  it('sorts active repos alphabetically, case-insensitive', () => {
    const repos = [
      makeRepo({ id: 'r1', name: 'Zebra', last_post_at: daysAgo(1) }),
      makeRepo({ id: 'r2', name: 'apple', last_post_at: daysAgo(1) }),
      makeRepo({ id: 'r3', name: 'Mango', last_post_at: daysAgo(1) }),
    ];
    const { active } = sortAndBucketRepos(repos);
    expect(active.map((r) => r.name)).toEqual(['apple', 'Mango', 'Zebra']);
  });

  it('sorts inactive repos alphabetically, case-insensitive', () => {
    const repos = [
      makeRepo({ id: 'r1', name: 'Zoo', last_post_at: null }),
      makeRepo({ id: 'r2', name: 'alpha', last_post_at: null }),
    ];
    const { inactive } = sortAndBucketRepos(repos);
    expect(inactive.map((r) => r.name)).toEqual(['alpha', 'Zoo']);
  });

  it('splits correctly with mixed active and inactive', () => {
    const repos = [
      makeRepo({ id: 'r1', name: 'B', last_post_at: daysAgo(1) }),
      makeRepo({ id: 'r2', name: 'A', last_post_at: null }),
      makeRepo({ id: 'r3', name: 'C', last_post_at: daysAgo(60) }),
    ];
    const { active, inactive } = sortAndBucketRepos(repos);
    expect(active.map((r) => r.id)).toEqual(['r1']);
    expect(inactive.map((r) => r.name)).toEqual(['A', 'C']);
  });
});
