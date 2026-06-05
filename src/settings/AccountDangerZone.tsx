// SPDX-License-Identifier: BUSL-1.1
// §22.7 — Account-level danger zone: Delete my Postlane account.

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import AccountDeletionProgress from './AccountDeletionProgress';

// ── Types ─────────────────────────────────────────────────────────────────────

interface Props { userEmail: string; onDeleted: () => void; }

// ── 22.7.7a: incomplete deletion warning ──────────────────────────────────────

function IncompleteWarning() {
  return (
    <article className="message is-danger mb-3" role="alert">
      <div className="message-body">
        <p>
          A previous account deletion was incomplete. Your project data and credentials have been
          removed from this device, but your Postlane account record on the server was not deleted.
          Retry the deletion below to complete the removal.
        </p>
      </div>
    </article>
  );
}

// ── 22.7.3: workspace deletion checkbox ──────────────────────────────────────

function WorkspaceDirCheckbox({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="checkbox mb-3" style={{ display: 'flex', gap: '0.5rem', alignItems: 'flex-start' }}>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>
        Also permanently delete all workspace directories and their contents (posts, drafts,
        and history files) from this machine.
        <span className="is-size-7 has-text-grey ml-1">
          This includes your sent post history (sent.jsonl). Recommended for privacy.
        </span>
      </span>
    </label>
  );
}

// ── 22.7.2: deletion confirmation modal ───────────────────────────────────────

interface DeleteModalProps {
  userEmail: string; onDeleted: () => void; onCancel: () => void;
  deleteWorkspaceDirsValue: boolean; onDeleteWorkspaceDirsChange: (v: boolean) => void;
}

function DeleteModal({ userEmail, onDeleted, onCancel, deleteWorkspaceDirsValue, onDeleteWorkspaceDirsChange }: DeleteModalProps) {
  const [emailInput, setEmailInput] = useState('');
  const emailMatches = emailInput.toLowerCase() === userEmail.toLowerCase();

  return (
    <div className="modal is-active" role="dialog" aria-label="Delete account">
      <div className="modal-background" onClick={onCancel} />
      <div className="modal-card">
        <header className="modal-card-head">
          <p className="modal-card-title has-text-danger">Delete my Postlane account</p>
        </header>
        <section className="modal-card-body">
          <p className="mb-3">
            This permanently deletes your account, all project data, and credentials from
            Postlane&apos;s servers. This cannot be undone.
          </p>
          <WorkspaceDirCheckbox checked={deleteWorkspaceDirsValue} onChange={onDeleteWorkspaceDirsChange} />
          <label className="label" htmlFor="account-delete-email">To confirm, type <strong>{userEmail}</strong>:</label>
          <input
            id="account-delete-email" className="input" type="email"
            placeholder="Type your account email to confirm"
            value={emailInput} onChange={(e) => setEmailInput(e.target.value)}
          />
        </section>
        <footer className="modal-card-foot" style={{ gap: '0.5rem' }}>
          <button className="button is-danger" onClick={onDeleted} disabled={!emailMatches}>
            Delete my account
          </button>
          <button className="button" onClick={onCancel}>Cancel</button>
        </footer>
      </div>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

type DangerView = 'idle' | 'confirming' | 'deleting';

export default function AccountDangerZone({ userEmail, onDeleted }: Props) {
  const [expanded, setExpanded] = useState(false);
  const [view, setView] = useState<DangerView>('idle');
  const [deleteWorkspaceDirs, setDeleteWorkspaceDirs] = useState(true);
  const [deletionIncomplete, setDeletionIncomplete] = useState(false);

  useEffect(() => {
    invoke<boolean>('get_deletion_incomplete').then(setDeletionIncomplete).catch(() => {});
  }, []);

  if (view === 'deleting') {
    return (
      <AccountDeletionProgress
        deleteWorkspaceDirs={deleteWorkspaceDirs}
        onDeleted={onDeleted}
        onAbort={() => setView('idle')}
      />
    );
  }

  return (
    <div className="mt-5">
      {deletionIncomplete && <IncompleteWarning />}
      <div style={{ border: '1px solid #f14668', borderRadius: 4 }}>
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
            <button className="button is-small is-danger" onClick={() => setView('confirming')}>
              Delete my Postlane account
            </button>
          </div>
        )}
      </div>
      {view === 'confirming' && (
        <DeleteModal
          userEmail={userEmail}
          onDeleted={() => setView('deleting')}
          onCancel={() => setView('idle')}
          onDeleteWorkspaceDirsChange={setDeleteWorkspaceDirs}
          deleteWorkspaceDirsValue={deleteWorkspaceDirs}
        />
      )}
    </div>
  );
}
