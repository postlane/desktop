// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '../ipc/invoke';
import WorkspaceMissingBanner, { useWorkspaceStatus } from './WorkspaceMissingBanner';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface LegacyRepoInfo {
  id: string;
  name: string;
  path: string;
}

export interface FieldConflict {
  field_key: string;
  label: string;
  repo_value: string;
  workspace_value: string;
}

export interface RepoConflicts {
  repo_path: string;
  repo_name: string;
  conflicts: FieldConflict[];
}

export interface MigrationStatus {
  qualifying_repos: LegacyRepoInfo[];
  /** All repos in the legacy array — controls Settings button (22.5.9). */
  total_legacy_repos: LegacyRepoInfo[];
  dismissed: boolean;
}

export interface RepoMigrationResult {
  repo_path: string;
  repo_name: string;
  status:
    | { tag: 'success'; posts_dir: string }
    | { tag: 'verification_failed'; error: string }
    | { tag: 'project_id_mismatch' }
    | { tag: 'skipped' };
}

export interface MigrationResult {
  results: RepoMigrationResult[];
}

export interface MigrationJournalEntry {
  repo_path: string;
  posts_dir: string;
  registry_updated: boolean;
  originals_deleted: boolean;
}

export interface JournalStatus {
  workspace_id: string;
  workspace_path: string;
  pending_entries: MigrationJournalEntry[];
  dismiss_count: number;
}

// ── Hook: migration status ────────────────────────────────────────────────────

export function useMigrationStatus() {
  const [status, setStatus] = useState<MigrationStatus | null>(null);

  useEffect(() => {
    invoke<MigrationStatus>('migration_status').then(setStatus).catch(() => {});
  }, []);

  const dismiss = useCallback(async () => {
    try {
      await invoke('dismiss_migration');
      setStatus(null);
    } catch { /* swallow */ }
  }, []);

  return { status, dismiss };
}

// ── Hook: journal statuses ────────────────────────────────────────────────────

export function useJournalStatuses() {
  const [statuses, setStatuses] = useState<JournalStatus[]>([]);

  useEffect(() => {
    invoke<JournalStatus[]>('get_journal_statuses').then(setStatuses).catch(() => {});
  }, []);

  const resume = useCallback(async (workspaceId: string) => {
    try {
      await invoke('resume_workspace_journal', { workspaceId });
      setStatuses((prev) => prev.filter((s) => s.workspace_id !== workspaceId));
    } catch { /* swallow */ }
  }, []);

  const dismissSession = useCallback(async (workspaceId: string) => {
    try {
      await invoke('dismiss_workspace_journal_session', { workspaceId });
      setStatuses((prev) => prev.filter((s) => s.workspace_id !== workspaceId));
    } catch { /* swallow */ }
  }, []);

  return { statuses, resume, dismissSession };
}

// ── Banner: migration ─────────────────────────────────────────────────────────

interface MigrationBannerProps {
  status: MigrationStatus;
  onDismiss: () => void;
  onSetupWorkspace: () => void;
}

export function MigrationBannerContent({ status, onDismiss, onSetupWorkspace }: MigrationBannerProps) {
  const count = status.qualifying_repos.length;
  if (count === 0) return null;

  return (
    <article className="message is-info mb-3" role="status" aria-label="Workspace migration available">
      <div className="message-body is-flex is-align-items-flex-start" style={{ gap: '0.75rem' }}>
        <div style={{ flex: 1 }}>
          <p className="is-size-6">
            Postlane now supports a central workspace for all your drafts. Migrate your existing posts
            to a workspace folder to keep everything in one place.
          </p>
          <p className="is-size-7 has-text-grey mt-1">
            {count} repositor{count === 1 ? 'y' : 'ies'} with posts found.
          </p>
        </div>
        <div className="is-flex" style={{ gap: '0.5rem', flexShrink: 0 }}>
          <button className="button is-small is-info" onClick={onSetupWorkspace}>
            Set up workspace
          </button>
          <button className="button is-small is-light" onClick={onDismiss}>
            Not now
          </button>
        </div>
      </div>
    </article>
  );
}

// ── Composite block used by OrgQueueView ─────────────────────────────────────

export function MigrationBannersBlock({ projectId }: { projectId: string }) {
  const { result: wsStatus, clearStatus } = useWorkspaceStatus(projectId);
  const [wsDismissed, setWsDismissed] = useState(false);
  const { status: migrationStatus, dismiss: dismissMigration } = useMigrationStatus();
  const { statuses: journalStatuses, resume: resumeJournal, dismissSession: dismissJournal } = useJournalStatuses();
  const [showMigrationFlow, setShowMigrationFlow] = useState(false);
  return (
    <>
      {wsStatus && wsStatus.status.tag !== 'ok' && !wsDismissed && (
        <WorkspaceMissingBanner result={wsStatus} onResolved={clearStatus}
          onDismiss={() => setWsDismissed(true)} />
      )}
      {migrationStatus && !migrationStatus.dismissed && migrationStatus.qualifying_repos.length > 0 && !showMigrationFlow && (
        <MigrationBannerContent
          status={migrationStatus}
          onDismiss={dismissMigration}
          onSetupWorkspace={() => setShowMigrationFlow(true)}
        />
      )}
      {journalStatuses.map((j) => (
        <RecoveryBannerContent
          key={j.workspace_id}
          journal={j}
          onResume={resumeJournal}
          onDismiss={dismissJournal}
        />
      ))}
    </>
  );
}

// ── Banner: crash recovery ────────────────────────────────────────────────────

interface RecoveryBannerProps {
  journal: JournalStatus;
  onResume: (workspaceId: string) => void;
  onDismiss: (workspaceId: string) => void;
}

export function RecoveryBannerContent({ journal, onResume, onDismiss }: RecoveryBannerProps) {
  const isNonDismissible = journal.dismiss_count >= 3;

  return (
    <article className="message is-warning mb-3" role="alert" aria-label="Migration recovery available">
      <div className="message-body is-flex is-align-items-flex-start" style={{ gap: '0.75rem' }}>
        <div style={{ flex: 1 }}>
          {isNonDismissible ? (
            <p className="is-size-6">
              Original files from a previous migration still exist in your repositories and cannot be
              cleaned up until you resume.
            </p>
          ) : (
            <p className="is-size-6">A previous migration was interrupted.</p>
          )}
        </div>
        <div className="is-flex" style={{ gap: '0.5rem', flexShrink: 0 }}>
          <button
            className="button is-small is-warning"
            onClick={() => onResume(journal.workspace_id)}
          >
            {isNonDismissible ? 'Resume cleanup now' : 'Resume cleanup'}
          </button>
          {!isNonDismissible && (
            <button className="button is-small is-light" onClick={() => onDismiss(journal.workspace_id)}>
              Dismiss
            </button>
          )}
        </div>
      </div>
    </article>
  );
}
