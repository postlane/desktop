// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { useAsyncCommand } from '../hooks/useAsyncCommand';
import { invoke } from '../ipc/invoke';
import VoiceGuideHint from './VoiceGuideHint';
import { RepoListSection } from './RepoTable';
import WorkspaceSetupWizard from '../wizard/workspace-setup/WorkspaceSetupWizard';
import type { AppStateFile } from '../types';
import type { RepoConnectionStatus, RowActions } from './RepoTable';
import { useMigrationStatus } from './MigrationBanner';
import MigrationFlow from './MigrationFlow';

// ── Types ─────────────────────────────────────────────────────────────────────

interface Props { projectId: string; projectName?: string; isOwner: boolean; }

interface RescanResult { added: string[]; deactivated: string[]; unchanged: string[]; }

// ── Confirm dialogs ───────────────────────────────────────────────────────────

function DisconnectConfirm({ onConfirm, onCancel, loading }: {
  onConfirm: () => void; onCancel: () => void; loading: boolean;
}) {
  return (
    <div className="mt-2 p-3 has-background-warning-light" style={{ borderRadius: 4 }}>
      <p className="is-size-7">
        This will remove Postlane&apos;s access to your GitHub organisation.
        Existing drafts are not deleted, but no new events will be received until the App is reinstalled.
      </p>
      <div className="is-flex mt-2" style={{ gap: '0.5rem' }}>
        <button className="button is-small is-danger" onClick={onConfirm} disabled={loading}>
          Confirm disconnect
        </button>
        <button className="button is-small" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}


function RescanResultView({ result }: { result: RescanResult }) {
  if (result.added.length === 0 && result.deactivated.length === 0) {
    return <p className="is-size-7 has-text-grey mt-2">All repos up to date.</p>;
  }
  return (
    <div className="mt-2 is-size-7">
      {result.added.length > 0 && <p className="has-text-success">Added: {result.added.length}</p>}
      {result.deactivated.length > 0 && <p className="has-text-warning">No longer found: {result.deactivated.length}</p>}
    </div>
  );
}

// ── Owner action bar ──────────────────────────────────────────────────────────

function OwnerActionBar({ hasGitHubApp, rescanScanning, disconnectPending,
  onAddWorkspace, onRescan, onDisconnect }: {
  hasGitHubApp: boolean; rescanScanning: boolean; disconnectPending: boolean;
  onAddWorkspace: () => void; onRescan: () => void; onDisconnect: () => void;
}) {
  return (
    <div className="is-flex mt-3" style={{ gap: '0.5rem', flexWrap: 'wrap' }}>
      <button className="button is-small is-success" onClick={onAddWorkspace}>Add workspace</button>
      <button className="button is-small is-success" onClick={onRescan} disabled={rescanScanning}>
        {rescanScanning ? 'Rescanning…' : 'Rescan workspace'}
      </button>
      {hasGitHubApp && !disconnectPending && (
        <button className="button is-small is-danger" onClick={onDisconnect}>
          Disconnect GitHub App
        </button>
      )}
    </div>
  );
}

// ── Hooks ─────────────────────────────────────────────────────────────────────

function useConnectionStatus(projectId: string) {
  const [rows, setRows] = useState<RepoConnectionStatus[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const data = await invoke<RepoConnectionStatus[]>('get_repo_connection_status', { projectId });
      setRows(Array.isArray(data) ? data : []);
    } catch {
      setRows([]);
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => { refresh(); }, [refresh]);
  return { rows, loading, refresh };
}

function useRepoActions(rows: RepoConnectionStatus[], refresh: () => void) {
  const [pendingRemoveId, setPendingRemoveId] = useState<string | null>(null);
  const { loading: removeLoading, run: runRemove } = useAsyncCommand();
  async function handleConfirmRemove() {
    if (!pendingRemoveId) return;
    const result = await runRemove(async () => { await invoke('unregister_repo', { repoId: pendingRemoveId }); return true; });
    if (result !== null) {
      setPendingRemoveId(null);
      refresh();
    }
  }

  const pendingName = rows.find((r) => r.repo_id === pendingRemoveId)?.display_name ?? '';
  return { pendingRemoveId, setPendingRemoveId, removeLoading, pendingName, handleConfirmRemove };
}

function useDisconnect(projectId: string, refresh: () => void) {
  const [pending, setPending] = useState(false);
  const { loading, run } = useAsyncCommand();

  async function confirm() {
    const result = await run(async () => { await invoke('disconnect_github_app', { projectId }); return true; });
    setPending(false);
    if (result !== null) {
      refresh();
      window.open('https://github.com/settings/installations', '_blank');
    }
  }

  return { pending, setPending, loading, confirm };
}


function useRescanWorkspace(workspaceId: string, refresh: () => void) {
  const { loading: scanning, error, run } = useAsyncCommand();
  const [result, setResult] = useState<RescanResult | null>(null);
  async function rescan() {
    const data = await run(() => invoke<RescanResult>('rescan_workspace', { workspaceId }));
    if (data !== null) {
      setResult(data);
      refresh();
    }
  }
  return { scanning, error, result, rescan };
}

function useWorkspacePath(projectId: string) {
  const [workspacePath, setWorkspacePath] = useState<string | null>(null);
  useEffect(() => {
    invoke<string | null>('get_workspace_path', { projectId })
      .then((p) => setWorkspacePath(p ?? null))
      .catch(() => {});
  }, [projectId]);
  return workspacePath;
}

function useVoiceGuideHintDismiss() {
  const [dismissed, setDismissed] = useState(false);
  const [appState, setAppState] = useState<AppStateFile | null>(null);

  useEffect(() => {
    invoke<AppStateFile>('get_app_state')
      .then((s) => { setAppState(s); setDismissed(s?.voice_guide_hint_dismissed ?? false); })
      .catch(() => {});
  }, []);

  async function dismiss() {
    setDismissed(true);
    if (!appState) return;
    const updated: AppStateFile = { ...appState, voice_guide_hint_dismissed: true };
    setAppState(updated);
    await invoke('save_app_state_command', { state: updated }).catch(() => {});
  }

  return { dismissed, dismiss };
}

// ── Migration re-entry (22.5.9 / 22.10.9) ────────────────────────────────────

function MigrateWorkspaceButton({ projectId }: { projectId: string }) {
  const { status } = useMigrationStatus();
  const [showFlow, setShowFlow] = useState(false);
  const hasLegacyRepos = (status?.total_legacy_repos.length ?? 0) > 0;
  if (!hasLegacyRepos) return null;
  if (showFlow) {
    return (
      <div className="mt-1">
        <MigrationFlow projectId={projectId} onDone={() => setShowFlow(false)} />
      </div>
    );
  }
  return (
    <button
      className="button is-small is-warning mt-2"
      onClick={() => {
        invoke('note_migration_reentered').catch(() => {});
        setShowFlow(true);
      }}
    >
      Migrate to workspace...
    </button>
  );
}

function OwnerStatusMessages({ rescanResult, rescanError }: {
  rescanResult: RescanResult | null; rescanError: string | null;
}) {
  return (
    <>
      {rescanResult && <RescanResultView result={rescanResult} />}
      {rescanError && <p role="alert" className="is-size-7 has-text-danger mt-2">{rescanError}</p>}
    </>
  );
}

// ── Setup wizard modal (checklist 24.3.7 -- Settings entry point) ────────────
//
// Repoints "Add workspace" from the old direct folder-picker ->
// add_workspace -> confirm_workspace_repos flow to the same 6-step
// WorkspaceSetupWizard the primary (left-nav) entry point uses, so both
// entry points write the same full config.json schema instead of two
// diverging shapes. add_workspace/confirm_workspace_repos/WorkspaceConfirmModal
// are left in place (still tested independently) for a future cleanup pass,
// not deleted here.

function SetupWizardModal({ projectId, projectName, onClose, onComplete }: {
  projectId: string; projectName: string; onClose: () => void; onComplete: () => void;
}) {
  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onClose} />
      <div className="modal-card" style={{ width: '90%', maxWidth: '640px' }}>
        <section className="modal-card-body">
          <WorkspaceSetupWizard
            projectId={projectId}
            projectName={projectName}
            onComplete={onComplete}
            onBack={onClose}
          />
        </section>
      </div>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function RepositoriesBlock({ projectId, projectName, isOwner }: Props) {
  const { rows, loading, refresh } = useConnectionStatus(projectId);
  const actions = useRepoActions(rows, refresh);
  const disconnect = useDisconnect(projectId, refresh);
  const rescan = useRescanWorkspace(projectId, refresh);
  const workspacePath = useWorkspacePath(projectId);
  const { dismissed: hintDismissed, dismiss: dismissHint } = useVoiceGuideHintDismiss();
  const [showSetupWizard, setShowSetupWizard] = useState(false);

  const hasGitHubApp = rows.some((r) => r.github_app_connected);

  const rowActions: RowActions = {
    pendingRemoveId: actions.pendingRemoveId,
    removeLoading: actions.removeLoading,
    onRemoveStart: actions.setPendingRemoveId,
    onConfirmRemove: actions.handleConfirmRemove,
    onCancelRemove: () => actions.setPendingRemoveId(null),
  };

  return (
    <div>
      <p className="is-size-6 has-text-weight-medium mb-3">Repositories</p>
      <RepoListSection loading={loading} rows={rows} isOwner={isOwner} actions={rowActions} />

      {isOwner && (
        <>
          <OwnerActionBar hasGitHubApp={hasGitHubApp}
            rescanScanning={rescan.scanning} disconnectPending={disconnect.pending}
            onAddWorkspace={() => setShowSetupWizard(true)}
            onRescan={rescan.rescan}
            onDisconnect={() => disconnect.setPending(true)} />
          {disconnect.pending && (
            <DisconnectConfirm onConfirm={disconnect.confirm}
              onCancel={() => disconnect.setPending(false)} loading={disconnect.loading} />
          )}
          <OwnerStatusMessages rescanResult={rescan.result} rescanError={rescan.error} />
          <MigrateWorkspaceButton projectId={projectId} />
        </>
      )}

      {workspacePath && !hintDismissed && (
        <VoiceGuideHint workspacePath={workspacePath} onDismiss={dismissHint} />
      )}

      {showSetupWizard && (
        <SetupWizardModal
          projectId={projectId}
          projectName={projectName ?? ''}
          onClose={() => setShowSetupWizard(false)}
          onComplete={() => { setShowSetupWizard(false); refresh(); }}
        />
      )}
    </div>
  );
}
