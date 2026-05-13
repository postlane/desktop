// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '../ipc/invoke';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

function repoConnectError(err: unknown, workspaceName?: string): string {
  const raw = typeof err === 'string' ? err : '';
  if (raw.startsWith('NotAGitRepo:')) return 'Not a Git repository. Please select a folder that contains a .git directory.';
  if (raw.startsWith('RepoAlreadyRegistered:')) {
    const target = workspaceName ? `the ${workspaceName} workspace` : 'a workspace';
    return `This repository is already connected to ${target}.`;
  }
  if (raw.startsWith('PathNotAuthorised:')) return 'This folder is outside your home directory and cannot be connected.';
  return 'Failed to connect repository';
}

function ModalBody({ connectedName, error }: { connectedName: string | null; error: string | null }) {
  if (connectedName) {
    return (
      <p className="is-size-7">
        <span className="tag is-success is-light mr-2">&#10003;</span>
        <strong>{connectedName}</strong> connected.
      </p>
    );
  }
  return (
    <>
      <p className="is-size-7 has-text-grey mb-3">Select a git repository folder to connect to this project.</p>
      {error && <p role="alert" className="is-size-7 has-text-danger">{error}</p>}
    </>
  );
}

function ModalFooter({ connectedName, loading, onDone, onCancel, onBrowse }: {
  connectedName: string | null; loading: boolean; onDone: () => void; onCancel: () => void; onBrowse: () => void;
}) {
  if (connectedName) return <button className="button is-primary" onClick={onDone}>Done</button>;
  return (
    <>
      <button className="button is-ghost" onClick={onCancel}>Cancel</button>
      <button className="button is-primary" onClick={onBrowse} disabled={loading}>
        {loading ? 'Adding…' : 'Browse for the folder'}
      </button>
    </>
  );
}

interface Props {
  onClose: () => void;
  projectId: string;
  projectName: string;
}

export default function AddRepoModal({ onClose, projectId, projectName }: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [connectedName, setConnectedName] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const pickerOpenRef = useRef(false);

  // Single guarded close used by every dismissal path (background, X, Cancel,
  // Escape) so phantom clicks from the OS folder picker can never slip through.
  function guardedClose() {
    if (pickerOpenRef.current || loading) return;
    onClose();
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !pickerOpenRef.current && !loading && !connectedName) onClose();
    };
    document.addEventListener('keydown', onKey);
    ref.current?.focus();
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose, loading, connectedName]);

  async function handleBrowse() {
    if (pickerOpenRef.current || loading) return;
    setError(null);
    // Set loading + ref before opening the dialog so guardedClose is armed
    // synchronously — no React re-render cycle needed.
    pickerOpenRef.current = true;
    setLoading(true);
    const selected = await openDialog({ directory: true });
    if (typeof selected !== 'string') {
      pickerOpenRef.current = false;
      setLoading(false);
      return;
    }
    try {
      const repo = await invoke<{ name: string }>('connect_repo_from_desktop', { repoPath: selected, projectId });
      setConnectedName(repo.name);
    } catch (err) {
      setError(repoConnectError(err, projectName));
    } finally {
      pickerOpenRef.current = false;
      setLoading(false);
    }
  }

  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={guardedClose} />
      <div className="modal-card" role="dialog" aria-modal="true" ref={ref} tabIndex={-1}>
        <header className="modal-card-head">
          <p className="modal-card-title">Add a repo</p>
          <button className="delete" onClick={guardedClose} aria-label="Close" />
        </header>
        <section className="modal-card-body">
          <ModalBody connectedName={connectedName} error={error} />
        </section>
        <footer className="modal-card-foot is-justify-content-flex-end" style={{ gap: '0.5rem' }}>
          <ModalFooter connectedName={connectedName} loading={loading} onDone={onClose} onCancel={guardedClose} onBrowse={handleBrowse} />
        </footer>
      </div>
    </div>
  );
}
