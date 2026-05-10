// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { RepoWithStatus } from '../types';

export interface RepoSummary {
  id: string;
  name: string;
  path: string;
  active: boolean;
}

/** Pre-M19 hook — returns all repos via `get_repos`. LeftNav uses this until 19.6 rebuilds
 *  the nav as org-centric. New M19 components should use `useProjectRepos` instead. */
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

/** M19 project-scoped hook — calls `list_repos_for_project` to return only repos
 *  belonging to the given project. Used by RepositoriesBlock (19.9.1) and any
 *  component that renders repos in an org-centric context. */
export function useProjectRepos(projectId: string) {
  const [repos, setRepos] = useState<RepoSummary[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const updated = await invoke<RepoSummary[]>('list_repos_for_project', { projectId });
      setRepos(updated);
    } catch (e) {
      console.error('Failed to refresh project repos:', e);
    }
  }, [projectId]);

  useEffect(() => {
    invoke<RepoSummary[]>('list_repos_for_project', { projectId })
      .then(setRepos)
      .catch((e) => {
        setLoadError('Could not load repositories. Check logs.');
        console.error('Failed to load project repos:', e);
      });
  }, [projectId]);

  return { repos, loadError, refresh };
}
