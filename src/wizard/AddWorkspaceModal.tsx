// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';

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
  if (msg.includes('No free project slot')) {
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
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', onKey);
    ref.current?.focus();
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose]);

  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onClose} />
      <div className="modal-card" role="dialog" aria-modal="true" ref={ref} tabIndex={-1}>
        <header className="modal-card-head">
          <p className="modal-card-title">Add a workspace</p>
          <button className="delete" onClick={onClose} aria-label="Close" />
        </header>
        <section className="modal-card-body">
          <p className="is-size-7 has-text-grey mb-4">A workspace holds your scheduler credentials and voice settings.</p>
          {error && <div role="alert" className="notification is-danger is-light is-size-7 mb-3">{error}</div>}
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
            <div className="field">
              <label htmlFor="ws-name" className="label is-small">Workspace name</label>
              <div className="control">
                <input id="ws-name" type="text" aria-label="Workspace name" value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="e.g. Postlane, Acme Corp, Personal" className="input is-small" />
              </div>
            </div>
            <div className="field">
              <label htmlFor="ws-type" className="label is-small">Workspace type</label>
              <div className="control">
                <div className="select is-small">
                  <select id="ws-type" aria-label="Workspace type" value={workspaceType}
                    onChange={(e) => setWorkspaceType(e.target.value as WorkspaceType)}>
                    <option value="personal">Personal</option>
                    <option value="organization">Organization</option>
                    <option value="client">Client project</option>
                  </select>
                </div>
              </div>
            </div>
          </div>
        </section>
        <footer className="modal-card-foot is-justify-content-flex-end" style={{ gap: '0.5rem' }}>
          <button className="button is-ghost" onClick={onClose}>Cancel</button>
          <button className="button is-primary" onClick={handleCreate} disabled={loading}>
            {loading ? 'Creating…' : 'Create workspace'}
          </button>
        </footer>
      </div>
    </div>
  );
}
