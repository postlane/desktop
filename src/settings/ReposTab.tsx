// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { Button } from '../components/catalyst/button';
import { Dialog, DialogActions, DialogDescription, DialogTitle } from '../components/catalyst/dialog';
import type { RepoWithStatus, SchedulerProfile } from '../types';
import RepoConfigureModal from './RepoConfigureModal';
import { PLATFORM_LABELS } from '../constants/platforms';

function platformLabel(platform: string): string {
  return PLATFORM_LABELS[platform] ?? platform;
}

interface ProfileSelectorProps {
  repoId: string;
  credentialVersion: number;
}

function ProfileSelector({ repoId, credentialVersion }: ProfileSelectorProps) {
  const [accounts, setAccounts] = useState<SchedulerProfile[]>([]);
  const [selected, setSelected] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<Record<string, string>>('get_account_ids', { repoId })
      .then((result) => setSelected(result ?? {}))
      .catch(() => {});
  }, [repoId]);

  useEffect(() => {
    setError(null);
    invoke<SchedulerProfile[]>('list_profiles_for_repo', { repoId })
      .then(setAccounts)
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
  }, [repoId, credentialVersion]);

  async function handleChange(platform: string, accountId: string) {
    setSelected((prev) => ({ ...prev, [platform]: accountId }));
    try {
      await invoke('save_account_id', { repoId, platform, accountId });
    } catch (e) {
      console.error('save_account_id failed:', e);
    }
  }

  const byPlatform = accounts.reduce<Record<string, SchedulerProfile[]>>((acc, profile) => {
    const platform = profile.platforms[0];
    if (platform) acc[platform] = [...(acc[platform] ?? []), profile];
    return acc;
  }, {});

  return (
    <div className="mt-3 border-t border-zinc-100 pt-3 dark:border-zinc-700">
      <p className="mb-2 text-xs font-medium text-zinc-600 dark:text-zinc-400">Posting accounts</p>
      {error ? (
        <p className="text-xs text-red-500">{error}</p>
      ) : accounts.length === 0 ? (
        <p className="text-xs text-zinc-400">No accounts connected. Add credentials in Settings → Scheduler.</p>
      ) : (
        <div className="space-y-2">
          {Object.keys(byPlatform).map((platform) => (
            <div key={platform} className="flex items-center gap-3">
              <span className="w-16 shrink-0 text-xs text-zinc-500">{platformLabel(platform)}</span>
              <select
                aria-label={`${platformLabel(platform)} account`}
                value={selected[platform] ?? ''}
                onChange={(e) => handleChange(platform, e.target.value)}
                className="flex-1 rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
              >
                <option value="">— select account —</option>
                {byPlatform[platform].map((a) => (
                  <option key={a.id} value={a.id}>{a.name}</option>
                ))}
              </select>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function RepoCard({ repo, togglingIds, onToggleActive, onUpdatePath, onRemoveConfirm }: {
  repo: RepoWithStatus;
  togglingIds: Set<string>;
  onToggleActive: (id: string, active: boolean) => void;
  onUpdatePath: (id: string) => void;
  onRemoveConfirm: (id: string) => void;
}) {
  const [credentialVersion, setCredentialVersion] = useState(0);
  const [configureOpen, setConfigureOpen] = useState(false);
  const isNotFound = !repo.path_exists;
  return (
    <div className="rounded-lg border border-zinc-200 p-4 dark:border-zinc-700">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="font-medium text-zinc-900 dark:text-zinc-100">{repo.name}</span>
            {isNotFound ? (
              <span title="not found" className="text-yellow-500">⚠</span>
            ) : repo.active ? (
              <span title="active" className="text-green-500">●</span>
            ) : (
              <span title="inactive" className="text-zinc-400">○</span>
            )}
          </div>
          <p className="mt-0.5 truncate text-xs text-zinc-500">
            {repo.path}{isNotFound && <span className="ml-1 text-yellow-600">(missing)</span>}
          </p>
        </div>
        <div className="flex shrink-0 gap-2">
          {isNotFound ? (
            <>
              <Button outline onClick={() => onUpdatePath(repo.id)}>Update path</Button>
              <Button outline onClick={() => onRemoveConfirm(repo.id)}>Remove</Button>
            </>
          ) : (
            <>
              <Button outline disabled={togglingIds.has(repo.id)} onClick={() => onToggleActive(repo.id, !repo.active)}>
                {repo.active ? 'Deactivate' : 'Activate'}
              </Button>
              <Button outline onClick={() => setConfigureOpen(true)}>Configure</Button>
              <Button outline onClick={() => onRemoveConfirm(repo.id)}>Remove</Button>
            </>
          )}
        </div>
      </div>
      {!isNotFound && <ProfileSelector repoId={repo.id} credentialVersion={credentialVersion} />}
      {configureOpen && <RepoConfigureModal repoId={repo.id} repoName={repo.name}
        currentProvider={repo.provider} onClose={() => setConfigureOpen(false)}
        onCredentialChange={() => setCredentialVersion((v) => v + 1)} />}
    </div>
  );
}

function RemoveRepoDialog({ id, onClose, onConfirm }: { id: string | null; onClose: () => void; onConfirm: (id: string) => void }) {
  return (
    <Dialog open={id !== null} onClose={onClose}>
      <DialogTitle>Remove repo</DialogTitle>
      <DialogDescription>This removes the repo from Postlane. Your files are not affected.</DialogDescription>
      <DialogActions>
        <Button plain onClick={onClose}>Cancel</Button>
        <Button color="red" onClick={() => id && onConfirm(id)}>Remove</Button>
      </DialogActions>
    </Dialog>
  );
}

interface ReposTabProps {
  onRepoChange: () => void;
  onAddWorkspace?: () => void;
  onAddRepo?: () => void;
}

function ReposHeader({ onAddWorkspace, onAdd }: { onAddWorkspace?: () => void; onAdd: () => void }) {
  return (
    <div className="flex items-center justify-between">
      <h2 className="text-sm font-semibold text-zinc-700 dark:text-zinc-300">Repos</h2>
      <div className="flex gap-2">
        {onAddWorkspace && <Button outline onClick={onAddWorkspace}>Add workspace</Button>}
        <Button onClick={onAdd}>Add repo</Button>
      </div>
    </div>
  );
}

export default function ReposTab({ onRepoChange, onAddWorkspace, onAddRepo }: ReposTabProps) {
  const [repos, setRepos] = useState<RepoWithStatus[]>([]);
  const [removeConfirmId, setRemoveConfirmId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [actionError, setActionError] = useState<string | null>(null);
  const [togglingIds, setTogglingIds] = useState<Set<string>>(new Set());

  const refresh = useCallback(async () => {
    try { const result = await invoke<RepoWithStatus[]>('get_repos'); setRepos(result); }
    catch (e) { console.error('get_repos failed:', e instanceof Error ? e.message : String(e)); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  async function handleAdd() {
    if (onAddRepo) { onAddRepo(); return; }
    setActionError(null);
    const selected = await openDialog({ directory: true });
    if (!selected) return;
    try { await invoke('add_repo', { path: selected }); refresh(); onRepoChange(); }
    catch (e) { setActionError(e instanceof Error ? e.message : 'Failed to add repo'); }
  }

  async function handleRemove(id: string) {
    setActionError(null);
    try { await invoke('remove_repo', { id }); setRemoveConfirmId(null); refresh(); onRepoChange(); }
    catch (e) { setActionError(e instanceof Error ? e.message : 'Failed to remove repo'); }
  }

  async function handleToggleActive(id: string, active: boolean) {
    if (togglingIds.has(id)) return;
    setTogglingIds((prev) => new Set(prev).add(id));
    setActionError(null);
    try { await invoke('set_repo_active', { id, active }); refresh(); }
    catch (e) { setActionError(e instanceof Error ? e.message : 'Failed to update repo'); }
    finally { setTogglingIds((prev) => { const next = new Set(prev); next.delete(id); return next; }); }
  }

  async function handleUpdatePath(id: string) {
    setActionError(null);
    const selected = await openDialog({ directory: true });
    if (!selected) return;
    try { await invoke('update_repo_path', { id, newPath: selected }); refresh(); }
    catch (e) { setActionError(e instanceof Error ? e.message : 'Failed to update repo path'); }
  }

  if (loading) return <p className="text-sm text-zinc-400">Loading…</p>;

  return (
    <div className="space-y-4">
      <ReposHeader onAddWorkspace={onAddWorkspace} onAdd={handleAdd} />
      <div className="space-y-2">
        {repos.map((repo) => (
          <RepoCard key={repo.id} repo={repo} togglingIds={togglingIds}
            onToggleActive={handleToggleActive} onUpdatePath={handleUpdatePath}
            onRemoveConfirm={setRemoveConfirmId}
          />
        ))}
        {repos.length === 0 && <p className="text-sm text-zinc-500">No repos registered. Add one to get started.</p>}
      </div>
      {actionError && <p className="mt-2 text-sm text-red-600 dark:text-red-400">{actionError}</p>}
      <RemoveRepoDialog id={removeConfirmId} onClose={() => setRemoveConfirmId(null)} onConfirm={handleRemove} />
    </div>
  );
}
