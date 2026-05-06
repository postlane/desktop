// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/catalyst/button';
import {
  Dialog, DialogActions, DialogBody, DialogDescription, DialogTitle,
} from '../components/catalyst/dialog';

interface Props {
  onClose: () => void;
  onCreated: () => void;
}

interface CreateProjectResult {
  project_id: string;
  name: string;
  workspace_type: string;
}

type WorkspaceType = 'personal' | 'organization' | 'client';

function apiErrorMessage(err: unknown): string {
  const msg = err instanceof Error ? err.message : String(err);
  if (msg.includes('no_free_slot')) {
    return 'You have no free workspace slot. Upgrade to add more workspaces.';
  }
  return `Failed to create workspace: ${msg}`;
}

function useAddWorkspaceForm(onCreated: () => void) {
  const [name, setName] = useState('');
  const [workspaceType, setWorkspaceType] = useState<WorkspaceType>('personal');
  const [validationError, setValidationError] = useState<string | null>(null);
  const [apiError, setApiError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleCreate() {
    setApiError(null);
    if (name.trim().length === 0) { setValidationError('Name is required.'); return; }
    if (name.trim().length > 64) { setValidationError('Name must be 64 characters or fewer.'); return; }
    setValidationError(null);
    setLoading(true);
    try {
      await invoke<CreateProjectResult>('create_project', { name: name.trim(), workspaceType });
      onCreated();
    } catch (err) {
      setApiError(apiErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }

  return {
    name, setName: (v: string) => { setName(v); setValidationError(null); },
    workspaceType, setWorkspaceType,
    error: validationError ?? apiError,
    loading, handleCreate,
  };
}

export default function AddWorkspaceModal({ onClose, onCreated }: Props) {
  const { name, setName, workspaceType, setWorkspaceType, error, loading, handleCreate } = useAddWorkspaceForm(onCreated);

  return (
    <Dialog open onClose={onClose}>
      <DialogTitle>Add a workspace</DialogTitle>
      <DialogDescription>A workspace holds your scheduler credentials and voice settings.</DialogDescription>
      <DialogBody>
        {error && (
          <div role="alert" className="mb-3 rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400">{error}</div>
        )}
        <div className="flex flex-col gap-4">
          <div>
            <label htmlFor="ws-name" className="mb-1 block text-sm font-medium text-zinc-700 dark:text-zinc-300">Workspace name</label>
            <input id="ws-name" type="text" aria-label="Workspace name" value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Postlane, Acme Corp, Personal" className="w-full rounded-lg border border-zinc-300 px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500" />
          </div>
          <div>
            <label htmlFor="ws-type" className="mb-1 block text-sm font-medium text-zinc-700 dark:text-zinc-300">Workspace type</label>
            <select id="ws-type" aria-label="Workspace type" value={workspaceType} onChange={(e) => setWorkspaceType(e.target.value as WorkspaceType)} className="w-full rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500">
              <option value="personal">Personal</option>
              <option value="organization">Organization</option>
              <option value="client">Client project</option>
            </select>
          </div>
        </div>
      </DialogBody>
      <DialogActions>
        <Button plain onClick={onClose}>Cancel</Button>
        <Button onClick={handleCreate} disabled={loading}>{loading ? 'Creating…' : 'Create workspace'}</Button>
      </DialogActions>
    </Dialog>
  );
}
