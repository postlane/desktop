// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import type { RepoWithStatus } from '../types';

const WATCHER_STALE_MS = 60_000;
const HEALTH_TICK_MS = 10_000;
const ACTIVITY_WINDOW_MS = 24 * 60 * 60 * 1000;

export function useWatcherHealth(repos: RepoWithStatus[], lastWatcherEvent: Map<string, Date>): Set<string> {
  const [stalledRepos, setStalledRepos] = useState<Set<string>>(new Set());

  useEffect(() => {
    const tick = () => {
      const now = Date.now();
      const stalled = new Set<string>();
      for (const repo of repos) {
        if (!repo.active || !repo.path_exists) continue;
        const lastEvent = lastWatcherEvent.get(repo.id);
        const elapsed = lastEvent ? now - lastEvent.getTime() : Infinity;
        const recentActivity = repo.last_post_at !== null && now - new Date(repo.last_post_at).getTime() < ACTIVITY_WINDOW_MS;
        if (elapsed > WATCHER_STALE_MS && recentActivity) stalled.add(repo.id);
      }
      setStalledRepos(stalled);
    };
    const id = setInterval(tick, HEALTH_TICK_MS);
    tick();
    return () => clearInterval(id);
  }, [repos, lastWatcherEvent]);

  return stalledRepos;
}
