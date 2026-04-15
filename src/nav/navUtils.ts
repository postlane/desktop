// SPDX-License-Identifier: BUSL-1.1

import type { RepoWithStatus, StatusIndicatorType } from '../types';

const THIRTY_DAYS_MS = 30 * 24 * 60 * 60 * 1000;

/** Parse an ISO 8601 string to a Date; returns null on invalid input. */
export function parseTimestamp(ts: string | null | undefined): Date | null {
  if (!ts) return null;
  const d = new Date(ts);
  return isNaN(d.getTime()) ? null : d;
}

/**
 * Returns true if the given last-activity date is within the last 30 days.
 * Repos with no activity (null) are considered inactive.
 */
export function isActive(lastActivity: Date | null): boolean {
  if (!lastActivity) return false;
  return Date.now() - lastActivity.getTime() < THIRTY_DAYS_MS;
}

/** Compute the status indicator for a repo row. */
export function getRepoStatus(repo: RepoWithStatus): StatusIndicatorType {
  if (!repo.path_exists) return { type: 'warning' };
  if (!repo.active) return { type: 'none' };

  const hasReady = repo.ready_count > 0;
  const hasFailed = repo.failed_count > 0;

  if (hasReady && hasFailed) return { type: 'stacked' };
  if (hasFailed) return { type: 'single', color: 'red' };
  if (hasReady) return { type: 'single', color: 'green' };
  return { type: 'none' };
}

/** Sort repos alphabetically, case-insensitive. */
function sortAlpha(repos: RepoWithStatus[]): RepoWithStatus[] {
  return [...repos].sort((a, b) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase()),
  );
}

export interface BucketedRepos {
  active: RepoWithStatus[];
  inactive: RepoWithStatus[];
}

/**
 * Splits repos into active (activity within 30 days) and inactive buckets,
 * each sorted alphabetically.
 */
export function sortAndBucketRepos(repos: RepoWithStatus[]): BucketedRepos {
  const active: RepoWithStatus[] = [];
  const inactive: RepoWithStatus[] = [];

  for (const repo of repos) {
    const lastActivity = parseTimestamp(repo.last_post_at);
    if (isActive(lastActivity)) {
      active.push(repo);
    } else {
      inactive.push(repo);
    }
  }

  return { active: sortAlpha(active), inactive: sortAlpha(inactive) };
}
