// SPDX-License-Identifier: BUSL-1.1

import { useEffect, type Dispatch, type SetStateAction } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { RepoWithStatus, AppStateFile, ViewSelection } from '../types';

export function useAppStateRestore(
  repos: RepoWithStatus[],
  setExpandedIds: Dispatch<SetStateAction<Set<string>>>,
  onNavigate: (_sel: ViewSelection) => void,
) {
  useEffect(() => {
    if (repos.length === 0) return;
    invoke<AppStateFile>('read_app_state_command')
      .then((appState) => {
        const validIds = appState.nav.expanded_repos.filter((id) => repos.some((r) => r.id === id));
        setExpandedIds(new Set(validIds));
        const lastRepoId = appState.nav.last_repo_id;
        const validViews = ['all_repos', 'repo'] as const;
        const validSections = ['drafts', 'published'] as const;
        const lastView = appState.nav.last_view;
        const lastSection = appState.nav.last_section;
        if (lastRepoId && repos.some((r) => r.id === lastRepoId) && (validViews as readonly string[]).includes(lastView) && (validSections as readonly string[]).includes(lastSection)) {
          onNavigate({ view: lastView as ViewSelection['view'], repoId: lastRepoId, section: lastSection as ViewSelection['section'] });
          setExpandedIds((prev) => new Set([...prev, lastRepoId]));
        }
      })
      .catch(() => { /* silently default to empty state on missing/corrupt app_state.json */ });
    // Only restore once when repos first load
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repos.length > 0]);
}
