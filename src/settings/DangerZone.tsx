// SPDX-License-Identifier: BUSL-1.1
// §22.6 — Workspace danger zone: Disconnect and Delete workspace.

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { useAsyncCommand } from '../hooks/useAsyncCommand';

// ── Types ─────────────────────────────────────────────────────────────────────

interface Props { workspaceId: string; isOwner: boolean; workspaceName?: string; onDisconnected?: () => void; onDeleted?: () => void; }
interface WorkspaceInfo { workspace_path: string; name: string; }
type Modal = 'none' | 'disconnect' | 'delete';
type DeleteStep = 'warning' | 'journal' | 'confirm';

// ── Hook ──────────────────────────────────────────────────────────────────────

function useWorkspaceInfo(workspaceId: string, workspaceName: string) {
  const [info, setInfo] = useState<WorkspaceInfo | null>(null);
  useEffect(() => {
    invoke<WorkspaceInfo>('get_workspace_info', { workspaceId })
      .then(setInfo)
      .catch(() => setInfo({ workspace_path: '', name: workspaceName }));
  }, [workspaceId, workspaceName]);
  return info;
}

// ── Disconnect modal (22.6.2) ─────────────────────────────────────────────────

function DisconnectModal({ workspaceId, name, onDone, onCancel, onDisconnected }: {
  workspaceId: string; name: string; onDone: () => void; onCancel: () => void;
  onDisconnected?: () => void;
}) {
  const { loading, error, run } = useAsyncCommand();

  async function handleConfirm() {
    const ok = await run(() => invoke('disconnect_workspace', { workspaceId }));
    if (ok !== null) { onDone(); onDisconnected?.(); }
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
          <button data-testid="modal-confirm-disconnect-btn" className="button is-danger" onClick={handleConfirm} disabled={loading}>Disconnect</button>
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

function DeleteConfirmStep({ workspaceId, info, onDone, onCancel, onDeleted }: {
  workspaceId: string; info: WorkspaceInfo; onDone: () => void; onCancel: () => void;
  onDeleted?: () => void;
}) {
  const [nameInput, setNameInput] = useState('');
  const { loading, error, run } = useAsyncCommand();
  const confirmed = nameInput === info.name;

  async function handleDelete() {
    const ok = await run(() => invoke('delete_workspace', { workspaceId }));
    if (ok !== null) { onDone(); onDeleted?.(); }
  }

  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onCancel} />
      <div className="modal-card">
        <header className="modal-card-head">
          <p className="modal-card-title has-text-danger">Confirm permanent deletion</p>
        </header>
        <section className="modal-card-body">
          {info.workspace_path && (
            <p className="is-family-monospace has-background-light p-2 mb-3">{info.workspace_path}</p>
          )}
          <label className="label" htmlFor="delete-confirm-input">To confirm, type the workspace name:</label>
          <input
            id="delete-confirm-input" aria-label="type the workspace name"
            className="input" type="text" value={nameInput}
            onChange={(e) => setNameInput(e.target.value)}
          />
          {error && <p className="has-text-danger mt-2">{error}</p>}
        </section>
        <footer className="modal-card-foot" style={{ gap: '0.5rem' }}>
          <button data-testid="modal-confirm-delete-btn" className="button is-danger" onClick={handleDelete} disabled={!confirmed || loading}>Delete</button>
          <button className="button" onClick={onCancel}>Cancel</button>
        </footer>
      </div>
    </div>
  );
}

// ── Delete modal — step machine ────────────────────────────────────────────────

function DeleteModal({ workspaceId, info, onDone, onCancel, onDeleted }: {
  workspaceId: string; info: WorkspaceInfo; onDone: () => void; onCancel: () => void;
  onDeleted?: () => void;
}) {
  const [step, setStep] = useState<DeleteStep>('warning');

  async function handleContinue() {
    const hasJournal = await invoke<boolean>('check_workspace_journal', { workspaceId }).catch(() => false);
    setStep(hasJournal ? 'journal' : 'confirm');
  }

  if (step === 'warning') return <DeleteWarningStep onContinue={handleContinue} onCancel={onCancel} />;
  if (step === 'journal') return <DeleteJournalStep onConfirm={() => setStep('confirm')} onCancel={onCancel} />;
  return <DeleteConfirmStep workspaceId={workspaceId} info={info} onDone={onDone} onCancel={onCancel} onDeleted={onDeleted} />;
}

// ── Main component (22.6.1 / 22.10.14 / 22.10.15) ───────────────────────────

export default function DangerZone({ workspaceId, isOwner, workspaceName = '', onDisconnected, onDeleted }: Props) {
  const info = useWorkspaceInfo(workspaceId, workspaceName);
  const [modal, setModal] = useState<Modal>('none');
  const [expanded, setExpanded] = useState(false);

  if (!isOwner) return null;

  return (
    <div style={{ border: '1px solid #f14668', borderRadius: 4, padding: '1rem' }}>
      <button
        className="button is-ghost is-fullwidth is-justify-content-flex-start has-text-danger has-text-weight-medium is-size-7 p-0"
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
      >
        Danger zone {expanded ? '▲' : '▼'}
      </button>
      {expanded && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem', marginTop: '0.75rem' }}>
          <div className="is-flex is-align-items-center is-justify-content-space-between">
            <span className="is-size-7">Disconnect this workspace</span>
            <button className="button is-warning is-size-7" onClick={() => setModal('disconnect')}>
              Disconnect
            </button>
          </div>
          <div className="is-flex is-align-items-center is-justify-content-space-between">
            <span className="is-size-7">Delete this workspace</span>
            <button className="button is-danger is-size-7" onClick={() => setModal('delete')}>
              Delete
            </button>
          </div>
        </div>
      )}
      {modal === 'disconnect' && (
        <DisconnectModal workspaceId={workspaceId} name={info?.name ?? workspaceId}
          onDone={() => setModal('none')} onCancel={() => setModal('none')}
          onDisconnected={onDisconnected} />
      )}
      {modal === 'delete' && info && (
        <DeleteModal workspaceId={workspaceId} info={info}
          onDone={() => setModal('none')} onCancel={() => setModal('none')} onDeleted={onDeleted} />
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
