// SPDX-License-Identifier: BUSL-1.1

import React, { createContext, useContext, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useDraftPosts, type DraftPostsState } from '../hooks/useDraftPosts';
import { DRAFT_DETECTED_EVENT } from '../constants/tauriEvents';

const DraftPostsContext = createContext<DraftPostsState | null>(null);

export function useDraftPostsContext(): DraftPostsState {
  const ctx = useContext(DraftPostsContext);
  if (ctx === null) {
    throw new Error('useDraftPostsContext must be called inside DraftPostsProvider');
  }
  return ctx;
}

export function DraftPostsProvider({ children }: { children: React.ReactNode }): React.ReactElement {
  const state = useDraftPosts();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen(DRAFT_DETECTED_EVENT, state.refresh).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [state.refresh]);

  return (
    <DraftPostsContext.Provider value={state}>
      {children}
    </DraftPostsContext.Provider>
  );
}
