// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

interface Props {
  onClose: () => void;
  projectId: string;
}

export default function AddRepoModal({ onClose, projectId }: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
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
    if (!selected) return;

    setLoading(true);
    try {
      const repo = await invoke<{ path: string }>('add_repo', { path: selected });
      if (projectId) {
        await invoke('write_project_id_to_config', { repoPath: repo.path, projectId });
      }
      onClose();
    } catch {
      setError("This folder hasn't been set up yet. Run `npx postlane init` inside it first.");
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
          <p className="is-size-7 has-text-grey mb-3">
            Select a folder where you've already run{' '}
            <code>npx postlane init</code>.
          </p>
          {error && <p className="is-size-7 has-text-danger">{error}</p>}
        </section>
        <footer className="modal-card-foot is-justify-content-flex-end" style={{ gap: '0.5rem' }}>
          <button className="button is-ghost" onClick={onClose}>Cancel</button>
          <button className="button is-primary" onClick={handleBrowse} disabled={loading}>
            {loading ? 'Adding…' : 'Browse for the folder'}
          </button>
        </footer>
      </div>
    </div>
  );
}
