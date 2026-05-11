// SPDX-License-Identifier: BUSL-1.1

import { useEffect, type Dispatch, type SetStateAction } from 'react';
import { invoke } from '../ipc/invoke';
import type { RepoWithStatus, AppStateFile, ViewSelection, OrgNavView, GlobalSettingsSection } from '../types';

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
        const sel = restoreViewSelection(
          appState.nav.last_view,
          appState.nav.last_repo_id,
          appState.nav.last_section,
        );
        if (sel) onNavigate(sel);
      })
      .catch(() => { /* silently default to empty state on missing/corrupt app_state.json */ });
    // Only restore once when repos first load
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repos.length > 0]);
}

function toOrgNavSection(s: string): OrgNavView {
  if (s === 'queue' || s === 'history' || s === 'settings') return s;
  return 'queue';
}

function toGlobalSection(s: string): GlobalSettingsSection {
  if (s === 'account' || s === 'preferences' || s === 'system') return s;
  return 'account';
}

function restoreViewSelection(
  lastView: string,
  lastProjectId: string | null,
  lastSection: string,
): ViewSelection | null {
  if (lastView === 'org_queue' && lastProjectId) return { view: 'org_queue', projectId: lastProjectId };
  if (lastView === 'org_history' && lastProjectId) return { view: 'org_history', projectId: lastProjectId };
  if (lastView === 'org_settings' && lastProjectId) {
    return { view: 'org_settings', projectId: lastProjectId, section: toOrgNavSection(lastSection) };
  }
  if (lastView === 'global_settings') {
    return { view: 'global_settings', section: toGlobalSection(lastSection) };
  }
  return null;
}
