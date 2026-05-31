// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '../ipc/invoke';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import WorkspaceConfirmModal from './WorkspaceConfirmModal';
import VoiceGuideHint from './VoiceGuideHint';
import { RepoListSection } from './RepoTable';
import type { RepoConnectionStatus, RowActions } from './RepoTable';
import type { WorkspaceSetupResult } from './WorkspaceConfirmModal';
import { useMigrationStatus } from './MigrationBanner';

// ── Types ─────────────────────────────────────────────────────────────────────

interface Props { projectId: string; isOwner: boolean; }

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
  const [removeLoading, setRemoveLoading] = useState(false);
  async function handleConfirmRemove() {
    if (!pendingRemoveId) return;
    setRemoveLoading(true);
    try {
      await invoke('unregister_repo', { repoId: pendingRemoveId });
      setPendingRemoveId(null);
      refresh();
    } finally {
      setRemoveLoading(false);
    }
  }

  const pendingName = rows.find((r) => r.repo_id === pendingRemoveId)?.display_name ?? '';
  return { pendingRemoveId, setPendingRemoveId, removeLoading, pendingName, handleConfirmRemove };
}

function useWorkspaceFlow(projectId: string, refresh: () => void) {
  const [wsResult, setWsResult] = useState<WorkspaceSetupResult | null>(null);
  const [wsError, setWsError] = useState<string | null>(null);
  const [wsToast, setWsToast] = useState<string | null>(null);
  const [wsSuccess, setWsSuccess] = useState<WorkspaceSetupResult | null>(null);

  async function handleAddWorkspace() {
    const selected = await openDialog({ directory: true });
    if (typeof selected !== 'string') return;
    setWsError(null);
    try {
      const result = await invoke<WorkspaceSetupResult>('add_workspace', { folderPath: selected, projectId });
      if (result.discovered_repos.length === 1) {
        const repoName = result.discovered_repos[0].name;
        setWsToast(`Creating workspace at ${result.workspace_path} — Postlane files will be added to ${repoName}/`);
        await invoke('confirm_workspace_repos', {
          workspaceId: result.workspace_id,
          selectedPaths: result.discovered_repos.map((r) => r.path),
        });
        setWsToast(null);
        setWsSuccess(result);
        refresh();
      } else {
        setWsResult(result);
      }
    } catch (err) {
      setWsToast(null);
      setWsError(typeof err === 'string' ? err : 'Failed to add workspace');
    }
  }

  async function handleConfirmWorkspace(selectedPaths: string[]) {
    if (!wsResult) return;
    try {
      await invoke('confirm_workspace_repos', { workspaceId: wsResult.workspace_id, selectedPaths });
      const completed = wsResult;
      setWsResult(null);
      setWsSuccess(completed);
      refresh();
    } catch (err) {
      setWsError(typeof err === 'string' ? err : 'Failed to confirm workspace');
    }
  }

  function dismissSuccess() { setWsSuccess(null); }

  return { wsResult, wsError, wsToast, wsSuccess, setWsResult, handleAddWorkspace, handleConfirmWorkspace, dismissSuccess };
}

function useDisconnect(projectId: string, refresh: () => void) {
  const [pending, setPending] = useState(false);
  const [loading, setLoading] = useState(false);

  async function confirm() {
    setLoading(true);
    try {
      await invoke('disconnect_github_app', { projectId });
      refresh();
      window.open('https://github.com/settings/installations', '_blank');
    } finally {
      setLoading(false);
      setPending(false);
    }
  }

  return { pending, setPending, loading, confirm };
}


function useRescanWorkspace(workspaceId: string) {
  const [scanning, setScanning] = useState(false);
  const [result, setResult] = useState<RescanResult | null>(null);
  async function rescan() {
    setScanning(true);
    try { setResult(await invoke<RescanResult>('rescan_workspace', { workspaceId })); }
    finally { setScanning(false); }
  }
  return { scanning, result, rescan };
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

// ── Migration re-entry (22.5.9) ───────────────────────────────────────────────

function MigrateWorkspaceButton() {
  const { status } = useMigrationStatus();
  const hasLegacyRepos = (status?.total_legacy_repos.length ?? 0) > 0;
  if (!hasLegacyRepos) return null;
  return (
    <button
      className="button is-small is-warning mt-2"
      onClick={() => invoke('note_migration_reentered').catch(() => {})}
    >
      Migrate to workspace...
    </button>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function RepositoriesBlock({ projectId, isOwner }: Props) {
  const { rows, loading, refresh } = useConnectionStatus(projectId);
  const actions = useRepoActions(rows, refresh);
  const disconnect = useDisconnect(projectId, refresh);
  const ws = useWorkspaceFlow(projectId, refresh);
  const rescan = useRescanWorkspace(projectId);
  const workspacePath = useWorkspacePath(projectId);

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
            onAddWorkspace={ws.handleAddWorkspace}
            onRescan={rescan.rescan}
            onDisconnect={() => disconnect.setPending(true)} />
          {disconnect.pending && (
            <DisconnectConfirm onConfirm={disconnect.confirm}
              onCancel={() => disconnect.setPending(false)} loading={disconnect.loading} />
          )}
          {rescan.result && <RescanResultView result={rescan.result} />}
          {ws.wsToast && <p className="is-size-7 has-text-info mt-2" role="status">{ws.wsToast}</p>}
          {ws.wsError && <p role="alert" className="is-size-7 has-text-danger mt-2">{ws.wsError}</p>}
          <MigrateWorkspaceButton />
        </>
      )}

      {ws.wsSuccess && (
        <VoiceGuideHint workspacePath={ws.wsSuccess.workspace_path} onDismiss={ws.dismissSuccess} />
      )}
      {!ws.wsSuccess && workspacePath && (
        <VoiceGuideHint workspacePath={workspacePath} />
      )}
      {ws.wsResult && (
        <WorkspaceConfirmModal result={ws.wsResult}
          onConfirm={ws.handleConfirmWorkspace}
          onCancel={() => ws.setWsResult(null)} />
      )}
    </div>
  );
}
