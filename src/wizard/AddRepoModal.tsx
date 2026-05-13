// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '../ipc/invoke';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

interface Props {
  onClose: () => void;
  projectId: string;
}

export default function AddRepoModal({ onClose, projectId }: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [connectedName, setConnectedName] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', onKey);
    ref.current?.focus();
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose]);

  async function handleBrowse() {
    setError(null);
    const selected = await openDialog({ directory: true });
    if (typeof selected !== 'string') return;

    setLoading(true);
    try {
      const repo = await invoke<{ name: string }>('connect_repo_from_desktop', { repoPath: selected, projectId });
      setConnectedName(repo.name);
    } catch (err) {
      const raw = typeof err === 'string' ? err : '';
      setError(raw.startsWith('NotAGitRepo:')
        ? 'Not a Git repository. Please select a folder that contains a .git directory.'
        : 'Failed to connect repository');
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onClose} />
      <div className="modal-card" role="dialog" aria-modal="true" ref={ref} tabIndex={-1}>
        <header className="modal-card-head">
          <p className="modal-card-title">Add a repo</p>
          <button className="delete" onClick={onClose} aria-label="Close" />
        </header>
        <section className="modal-card-body">
          {connectedName ? (
            <p className="is-size-7">
              <span className="tag is-success is-light mr-2">&#10003;</span>
              <strong>{connectedName}</strong> connected.
            </p>
          ) : (
            <>
              <p className="is-size-7 has-text-grey mb-3">
                Select a git repository folder to connect to this project.
              </p>
              {error && <p role="alert" className="is-size-7 has-text-danger">{error}</p>}
            </>
          )}
        </section>
        <footer className="modal-card-foot is-justify-content-flex-end" style={{ gap: '0.5rem' }}>
          {connectedName ? (
            <button className="button is-primary" onClick={onClose}>Done</button>
          ) : (
            <>
              <button className="button is-ghost" onClick={onClose}>Cancel</button>
              <button className="button is-primary" onClick={handleBrowse} disabled={loading}>
                {loading ? 'Adding…' : 'Browse for the folder'}
              </button>
            </>
          )}
        </footer>
      </div>
    </div>
  );
}
