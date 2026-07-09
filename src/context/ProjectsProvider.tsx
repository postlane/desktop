// SPDX-License-Identifier: BUSL-1.1

import React, { createContext, useContext, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '../ipc/invoke';
import { useProjects, type ProjectsState } from '../hooks/useProjects';
import { PROJECTS_CHANGED_EVENT, DEEP_LINK_NEW_URL_EVENT } from '../constants/tauriEvents';

interface ClassifiedDeepLink { kind: string; project_id: string | null; }

const ProjectsContext = createContext<ProjectsState | null>(null);

export function useProjectsContext(): ProjectsState {
  const ctx = useContext(ProjectsContext);
  if (ctx === null) {
    throw new Error('useProjectsContext must be called inside ProjectsProvider');
  }
  return ctx;
}

async function handleDeepLinkUrls(urls: string[], refresh: () => void) {
  for (const url of urls) {
    try {
      const classified = await invoke<ClassifiedDeepLink>('classify_deep_link', { url });
      if (classified.kind === 'billing_complete') refresh();
    } catch (e) {
      console.error('[projects-provider] failed to classify deep link:', e instanceof Error ? e.message : String(e));
    }
  }
}

export function ProjectsProvider({ children }: { children: React.ReactNode }): React.ReactElement {
  const state = useProjects();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen(PROJECTS_CHANGED_EVENT, state.refresh).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [state.refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<string[]>(DEEP_LINK_NEW_URL_EVENT, (event) => {
      handleDeepLinkUrls(event.payload, state.refresh);
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [state.refresh]);

  return (
    <ProjectsContext.Provider value={state}>
      {children}
    </ProjectsContext.Provider>
  );
}
