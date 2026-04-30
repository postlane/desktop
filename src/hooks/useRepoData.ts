// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { RepoWithStatus } from '../types';

export function useRepoData() {
  const [repos, setRepos] = useState<RepoWithStatus[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try { const updated = await invoke<RepoWithStatus[]>('get_repos'); setRepos(updated); }
    catch (e) { console.error('Failed to refresh repos:', e); }
  }, []);

  useEffect(() => {
    invoke<RepoWithStatus[]>('get_repos')
      .then(setRepos)
      .catch((e) => { setLoadError('Could not load repositories. Check logs.'); console.error('Failed to load repos:', e); });
  }, []);

  return { repos, loadError, refresh };
}
