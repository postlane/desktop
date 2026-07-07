// SPDX-License-Identifier: BUSL-1.1
// checklist 24.4.14/24.4.14a — list/promote/demote/remove a workspace's collaborators.

import { useState, useCallback, useEffect } from 'react';
import { invoke } from '../ipc/invoke';

export interface Collaborator {
  user_id: string;
  role: string;
  added_at: string;
  display_name: string | null;
  avatar_url: string | null;
}

function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export function useCollaborators(projectId: string) {
  const [collaborators, setCollaborators] = useState<Collaborator[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<Collaborator[]>('list_project_collaborators', { projectId });
      setCollaborators(Array.isArray(data) ? data : []);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => { refresh(); }, [refresh]);

  const setRole = useCallback(async (userId: string, role: 'admin' | 'member') => {
    setActionError(null);
    try {
      await invoke('update_collaborator_role', { projectId, userId, role });
      await refresh();
    } catch (e) {
      setActionError(errorMessage(e));
    }
  }, [projectId, refresh]);

  const remove = useCallback(async (userId: string) => {
    setActionError(null);
    try {
      await invoke('remove_project_collaborator', { projectId, userId });
      await refresh();
    } catch (e) {
      setActionError(errorMessage(e));
    }
  }, [projectId, refresh]);

  return { collaborators, loading, error, actionError, refresh, setRole, remove };
}
