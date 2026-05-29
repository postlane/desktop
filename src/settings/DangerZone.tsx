// SPDX-License-Identifier: BUSL-1.1
// §22.6 — Workspace danger zone: Disconnect and Delete workspace.

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';

// ── Types ─────────────────────────────────────────────────────────────────────

interface Props { workspaceId: string; isOwner: boolean; }
interface WorkspaceInfo { workspace_path: string; name: string; }
type Modal = 'none' | 'disconnect' | 'delete';
type DeleteStep = 'warning' | 'journal' | 'confirm';

// ── Hook ──────────────────────────────────────────────────────────────────────

function useWorkspaceInfo(workspaceId: string) {
  const [info, setInfo] = useState<WorkspaceInfo | null>(null);
  useEffect(() => {
    invoke<WorkspaceInfo>('get_workspace_info', { workspaceId })
      .then(setInfo)
      .catch(() => {});
  }, [workspaceId]);
  return info;
}

// ── Disconnect modal (22.6.2) ─────────────────────────────────────────────────

function DisconnectModal({ workspaceId, name, onDone, onCancel }: {
  workspaceId: string; name: string; onDone: () => void; onCancel: () => void;
}) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleConfirm() {
    setLoading(true);
    setError(null);
    try {
      await invoke('disconnect_workspace', { workspaceId });
      onDone();
    } catch (e) {
      setError(typeof e === 'string' ? e : 'Disconnect failed');
    } finally { setLoading(false); }
  }

  return (
    <div className="modal is-active" role="dialog" aria-label="Disconnect workspace">
      <div className="modal-background" onClick={onCancel} />
      <div className="modal-card">
        <header className="modal-card-head">
          <p className="modal-card-title">Disconnect {name}?</p>
        </header>
        <section className="modal-card-body">
          <p>This removes the workspace from Postlane but leaves your files intact.</p>
          {error && <p className="has-text-danger mt-2">{error}</p>}
        </section>
        <footer className="modal-card-foot" style={{ gap: '0.5rem' }}>
          <button className="button is-danger" onClick={handleConfirm} disabled={loading}>Disconnect</button>
          <button className="button" onClick={onCancel}>Cancel</button>
        </footer>
      </div>
    </div>
  );
}

// ── Delete modal — step sub-components (22.6.11/22.6.12a) ────────────────────

function DeleteWarningStep({ onContinue, onCancel }: { onContinue: () => void; onCancel: () => void }) {
  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onCancel} />
      <div className="modal-card">
        <header className="modal-card-head">
          <p className="modal-card-title has-text-danger">Delete workspace and all content?</p>
        </header>
        <section className="modal-card-body">
          <p>This permanently deletes all drafts and post history for this workspace, and removes
             the ability to draft or schedule posts from it. This cannot be undone.</p>
        </section>
        <footer className="modal-card-foot" style={{ gap: '0.5rem' }}>
          <button className="button is-danger" onClick={onContinue}>Continue</button>
          <button className="button" onClick={onCancel}>Cancel</button>
        </footer>
      </div>
    </div>
  );
}

function DeleteJournalStep({ onConfirm, onCancel }: { onConfirm: () => void; onCancel: () => void }) {
  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onCancel} />
      <div className="modal-card">
        <header className="modal-card-head">
          <p className="modal-card-title has-text-warning">Migration in progress</p>
        </header>
        <section className="modal-card-body">
          <p>A migration is in progress for this workspace. Deleting will permanently abandon
             the cleanup, and original files may still exist in your repositories.</p>
        </section>
        <footer className="modal-card-foot" style={{ gap: '0.5rem' }}>
          <button className="button is-danger" onClick={onConfirm}>I understand</button>
          <button className="button" onClick={onCancel}>Cancel</button>
        </footer>
      </div>
    </div>
  );
}

function DeleteConfirmStep({ workspaceId, info, onDone, onCancel }: {
  workspaceId: string; info: WorkspaceInfo; onDone: () => void; onCancel: () => void;
}) {
  const [nameInput, setNameInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const confirmed = nameInput === info.name;

  async function handleDelete() {
    setLoading(true);
    setError(null);
    try {
      await invoke('delete_workspace', { workspaceId });
      onDone();
    } catch (e) {
      setError(typeof e === 'string' ? e : 'Delete failed');
    } finally { setLoading(false); }
  }

  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onCancel} />
      <div className="modal-card">
        <header className="modal-card-head">
          <p className="modal-card-title has-text-danger">Confirm permanent deletion</p>
        </header>
        <section className="modal-card-body">
          <p className="is-family-monospace has-background-light p-2 mb-3">{info.workspace_path}</p>
          <label className="label" htmlFor="delete-confirm-input">To confirm, type the workspace name:</label>
          <input
            id="delete-confirm-input" aria-label="type the workspace name"
            className="input" type="text" value={nameInput}
            onChange={(e) => setNameInput(e.target.value)}
          />
          {error && <p className="has-text-danger mt-2">{error}</p>}
        </section>
        <footer className="modal-card-foot" style={{ gap: '0.5rem' }}>
          <button className="button is-danger" onClick={handleDelete} disabled={!confirmed || loading}>Delete</button>
          <button className="button" onClick={onCancel}>Cancel</button>
        </footer>
      </div>
    </div>
  );
}

// ── Delete modal — step machine ────────────────────────────────────────────────

function DeleteModal({ workspaceId, info, onDone, onCancel }: {
  workspaceId: string; info: WorkspaceInfo; onDone: () => void; onCancel: () => void;
}) {
  const [step, setStep] = useState<DeleteStep>('warning');

  async function handleContinue() {
    const hasJournal = await invoke<boolean>('check_workspace_journal', { workspaceId }).catch(() => false);
    setStep(hasJournal ? 'journal' : 'confirm');
  }

  if (step === 'warning') return <DeleteWarningStep onContinue={handleContinue} onCancel={onCancel} />;
  if (step === 'journal') return <DeleteJournalStep onConfirm={() => setStep('confirm')} onCancel={onCancel} />;
  return <DeleteConfirmStep workspaceId={workspaceId} info={info} onDone={onDone} onCancel={onCancel} />;
}

// ── Main component (22.6.1) ───────────────────────────────────────────────────

export default function DangerZone({ workspaceId, isOwner }: Props) {
  const info = useWorkspaceInfo(workspaceId);
  const [expanded, setExpanded] = useState(false);
  const [modal, setModal] = useState<Modal>('none');

  if (!isOwner) return null;

  return (
    <div className="mt-4" style={{ border: '1px solid #f14668', borderRadius: 4 }}>
      <button
        className="button is-ghost has-text-danger is-fullwidth"
        style={{ justifyContent: 'flex-start', padding: '0.5rem 1rem' }}
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
      >
        Danger Zone {expanded ? '▲' : '▼'}
      </button>
      {expanded && (
        <div className="p-3">
          <div className="is-flex" style={{ gap: '0.5rem', flexWrap: 'wrap' }}>
            <button className="button is-small is-light has-text-danger" onClick={() => setModal('disconnect')}>
              Disconnect this workspace
            </button>
            <button className="button is-small is-danger" onClick={() => setModal('delete')}>
              Delete workspace and all content
            </button>
          </div>
        </div>
      )}
      {modal === 'disconnect' && (
        <DisconnectModal workspaceId={workspaceId} name={info?.name ?? workspaceId}
          onDone={() => setModal('none')} onCancel={() => setModal('none')} />
      )}
      {modal === 'delete' && info && (
        <DeleteModal workspaceId={workspaceId} info={info}
          onDone={() => setModal('none')} onCancel={() => setModal('none')} />
      )}
      {modal === 'delete' && !info && (
        <div className="modal is-active">
          <div className="modal-background" onClick={() => setModal('none')} />
          <div className="modal-card"><section className="modal-card-body">Loading…</section></div>
        </div>
      )}
    </div>
  );
}
