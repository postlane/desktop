// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '../ipc/invoke';
import type { Project } from '../types';

export type { Project };

export interface ProjectsState {
  projects: Project[];
  loading: boolean;
  error: string | null;
  refresh: () => void;
  clear: () => void;
}

export function useProjects(): ProjectsState {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const seqRef = useRef(0);

  const load = useCallback(() => {
    const seq = ++seqRef.current;
    setLoading(true);
    setError(null);
    invoke<Project[]>('list_projects')
      .then((data) => {
        if (seqRef.current !== seq) return;
        setProjects(Array.isArray(data) ? data : []);
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
    setProjects([]);
    setLoading(false);
    setError(null);
  }, []);

  useEffect(() => { load(); }, [load]);

  return { projects, loading, error, refresh, clear };
}
