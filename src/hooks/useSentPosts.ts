// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '../ipc/invoke';
import type { PublishedPost } from '../types';

export interface SentPostsState {
  posts: PublishedPost[];
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

export function useSentPosts(projectId: string): SentPostsState {
  const [posts, setPosts] = useState<PublishedPost[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const seqRef = useRef(0);

  const load = useCallback(() => {
    const seq = ++seqRef.current;
    setLoading(true);
    setError(null);
    invoke<PublishedPost[]>('get_org_published', { projectId })
      .then((data) => {
        if (seqRef.current !== seq) return;
        setPosts(data);
        setLoading(false);
      })
      .catch((e: unknown) => {
        if (seqRef.current !== seq) return;
        setError(String(e));
        setLoading(false);
      });
  }, [projectId]);

  const refresh = useCallback(() => { load(); }, [load]);

  useEffect(() => { load(); }, [load]);

  return { posts, loading, error, refresh };
}
