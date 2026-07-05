// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '../ipc/invoke';

export interface ProviderAccountSummary {
  id: string;
  provider: string;
  provider_account_id: string | null;
  label: string | null;
  is_primary: boolean;
}

export interface ProviderAccountsState {
  accounts: ProviderAccountSummary[];
  activeAccountId: string | null;
  setActiveAccountId: (_id: string) => void;
  loading: boolean;
  error: string | null;
  refresh: () => void;
  clear: () => void;
}

function defaultActiveId(accounts: ProviderAccountSummary[]): string | null {
  return accounts.find((a) => a.is_primary)?.id ?? accounts[0]?.id ?? null;
}

export function useProviderAccounts(): ProviderAccountsState {
  const [accounts, setAccounts] = useState<ProviderAccountSummary[]>([]);
  const [activeAccountId, setActiveAccountId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const seqRef = useRef(0);

  const load = useCallback(() => {
    const seq = ++seqRef.current;
    setLoading(true);
    setError(null);
    invoke<ProviderAccountSummary[]>('list_provider_accounts')
      .then((data) => {
        if (seqRef.current !== seq) return;
        const list = Array.isArray(data) ? data : [];
        setAccounts(list);
        setActiveAccountId((current) => current ?? defaultActiveId(list));
        setLoading(false);
      })
      .catch((e: unknown) => {
        if (seqRef.current !== seq) return;
        setError(String(e));
        setLoading(false);
      });
  }, []);

  const refresh = useCallback(() => { load(); }, [load]);

  const clear = useCallback(() => {
    ++seqRef.current;
    setAccounts([]);
    setActiveAccountId(null);
    setLoading(false);
    setError(null);
  }, []);

  useEffect(() => { load(); }, [load]);

  return { accounts, activeAccountId, setActiveAccountId, loading, error, refresh, clear };
}
