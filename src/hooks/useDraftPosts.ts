// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '../ipc/invoke';
import type { DraftPost } from '../types';

export interface DraftPostsState {
  drafts: DraftPost[];
  loading: boolean;
  error: string | null;
  refresh: () => void;
  clear: () => void;
}

export function useDraftPosts(): DraftPostsState {
  const [drafts, setDrafts] = useState<DraftPost[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const seqRef = useRef(0);

  const load = useCallback(() => {
    const seq = ++seqRef.current;
    setLoading(true);
    setError(null);
    // v1: cross-repo load. Replace with get_org_drafts(project_id) when scale requires it.
    invoke<DraftPost[]>('get_all_drafts')
      .then((data) => {
        if (seqRef.current !== seq) return;
        setDrafts(Array.isArray(data) ? data : []);
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
    setDrafts([]);
    setLoading(false);
    setError(null);
  }, []);

  useEffect(() => { load(); }, [load]);

  return { drafts, loading, error, refresh, clear };
}
