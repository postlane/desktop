// SPDX-License-Identifier: BUSL-1.1

import React, { createContext, useContext, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useProviderAccounts, type ProviderAccountsState } from '../hooks/useProviderAccounts';

const ProviderAccountsContext = createContext<ProviderAccountsState | null>(null);

export function useProviderAccountsContext(): ProviderAccountsState {
  const ctx = useContext(ProviderAccountsContext);
  if (ctx === null) {
    throw new Error('useProviderAccountsContext must be called inside ProviderAccountsProvider');
  }
  return ctx;
}

export function ProviderAccountsProvider({ children }: { children: React.ReactNode }): React.ReactElement {
  const state = useProviderAccounts();
  const { refresh } = state;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<{ display_name: string; account_linked?: boolean }>('license:activated', (e) => {
      if (e.payload.account_linked) refresh();
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [refresh]);

  return (
    <ProviderAccountsContext.Provider value={state}>
      {children}
    </ProviderAccountsContext.Provider>
  );
}
