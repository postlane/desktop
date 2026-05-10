// SPDX-License-Identifier: BUSL-1.1

import React, { createContext, useContext, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useProjects, type ProjectsState } from '../hooks/useProjects';
import { PROJECTS_CHANGED_EVENT } from '../constants/tauriEvents';

const ProjectsContext = createContext<ProjectsState | null>(null);

export function useProjectsContext(): ProjectsState {
  const ctx = useContext(ProjectsContext);
  if (ctx === null) {
    throw new Error('useProjectsContext must be called inside ProjectsProvider');
  }
  return ctx;
}

export function ProjectsProvider({ children }: { children: React.ReactNode }): React.ReactElement {
  const state = useProjects();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen(PROJECTS_CHANGED_EVENT, state.refresh).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [state.refresh]);

  return (
    <ProjectsContext.Provider value={state}>
      {children}
    </ProjectsContext.Provider>
  );
}
