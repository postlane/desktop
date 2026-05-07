// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
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
    <div className="mt-3 pt-3" style={{ borderTop: '1px solid var(--bulma-border-weak)' }}>
      <p className="is-size-7 has-text-weight-medium has-text-grey mb-2">Posting accounts</p>
      {error ? (
        <p className="is-size-7 has-text-danger">{error}</p>
      ) : accounts.length === 0 ? (
        <p className="is-size-7 has-text-grey-light">No accounts connected. Add credentials in Settings → Scheduler.</p>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
          {Object.keys(byPlatform).map((platform) => (
            <div key={platform} className="is-flex is-align-items-center" style={{ gap: '0.75rem' }}>
              <span className="is-size-7 has-text-grey" style={{ width: '4rem', flexShrink: 0 }}>{platformLabel(platform)}</span>
              <div className="select is-small" style={{ flex: 1 }}>
                <select
                  aria-label={`${platformLabel(platform)} account`}
                  value={selected[platform] ?? ''}
                  onChange={(e) => handleChange(platform, e.target.value)}
                  style={{ width: '100%' }}
                >
                  <option value="">— select account —</option>
                  {byPlatform[platform].map((a) => (
                    <option key={a.id} value={a.id}>{a.name}</option>
                  ))}
                </select>
              </div>
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
    <div className="box p-4">
      <div className="is-flex is-align-items-flex-start is-justify-content-space-between" style={{ gap: '1rem' }}>
        <div style={{ minWidth: 0, flex: 1 }}>
          <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
            <span className="has-text-weight-medium">{repo.name}</span>
            {isNotFound
              ? <span title="not found" className="has-text-warning">⚠</span>
              : repo.active
              ? <span title="active" className="has-text-success">●</span>
              : <span title="inactive" className="has-text-grey-light">○</span>}
          </div>
          <p className="is-size-7 has-text-grey mt-1" style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {repo.path}{isNotFound && <span className="has-text-warning ml-1">(missing)</span>}
          </p>
        </div>
        <div className="is-flex" style={{ gap: '0.5rem', flexShrink: 0 }}>
          {isNotFound ? (
            <>
              <button className="button is-outlined is-small" onClick={() => onUpdatePath(repo.id)}>Update path</button>
              <button className="button is-outlined is-small" onClick={() => onRemoveConfirm(repo.id)}>Remove</button>
            </>
          ) : (
            <>
              <button className="button is-outlined is-small" disabled={togglingIds.has(repo.id)} onClick={() => onToggleActive(repo.id, !repo.active)}>
                {repo.active ? 'Deactivate' : 'Activate'}
              </button>
              <button className="button is-outlined is-small" onClick={() => setConfigureOpen(true)}>Configure</button>
              <button className="button is-outlined is-small" onClick={() => onRemoveConfirm(repo.id)}>Remove</button>
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
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!id) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', onKey);
    ref.current?.focus();
    return () => document.removeEventListener('keydown', onKey);
  }, [id, onClose]);

  if (!id) return null;
  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onClose} />
      <div className="modal-card" role="dialog" aria-modal="true" ref={ref} tabIndex={-1}>
        <header className="modal-card-head">
          <p className="modal-card-title">Remove repo</p>
          <button className="delete" onClick={onClose} aria-label="Close" />
        </header>
        <section className="modal-card-body">
          <p className="is-size-7">This removes the repo from Postlane. Your files are not affected.</p>
        </section>
        <footer className="modal-card-foot is-justify-content-flex-end" style={{ gap: '0.5rem' }}>
          <button className="button is-ghost" onClick={onClose}>Cancel</button>
          <button className="button is-danger" onClick={() => onConfirm(id)}>Remove</button>
        </footer>
      </div>
    </div>
  );
}

interface ReposTabProps {
  onRepoChange: () => void;
  onAddWorkspace?: () => void;
  onAddRepo?: () => void;
}

function ReposHeader({ onAddWorkspace, onAdd }: { onAddWorkspace?: () => void; onAdd: () => void }) {
  return (
    <div className="is-flex is-align-items-center is-justify-content-space-between mb-4">
      <h2 className="has-text-weight-semibold is-size-7">Repos</h2>
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        {onAddWorkspace && <button className="button is-outlined is-small" onClick={onAddWorkspace}>Add workspace</button>}
        <button className="button is-primary is-small" onClick={onAdd}>Add repo</button>
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

  if (loading) return <p className="is-size-7 has-text-grey">Loading…</p>;

  return (
    <>
      <div aria-hidden={removeConfirmId !== null ? 'true' : undefined}
        style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
        <ReposHeader onAddWorkspace={onAddWorkspace} onAdd={handleAdd} />
        {repos.map((repo) => (
          <RepoCard key={repo.id} repo={repo} togglingIds={togglingIds}
            onToggleActive={handleToggleActive} onUpdatePath={handleUpdatePath}
            onRemoveConfirm={setRemoveConfirmId}
          />
        ))}
        {repos.length === 0 && <p className="is-size-7 has-text-grey">No repos registered. Add one to get started.</p>}
        {actionError && <p className="is-size-7 has-text-danger mt-2">{actionError}</p>}
      </div>
      <RemoveRepoDialog id={removeConfirmId} onClose={() => setRemoveConfirmId(null)} onConfirm={handleRemove} />
    </>
  );
}
