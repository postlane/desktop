// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { useProjectRepos } from '../hooks/useRepoData';
import type { RepoSummary } from '../hooks/useRepoData';
import AddRepoModal from '../wizard/AddRepoModal';
import RepoConfigureModal from './RepoConfigureModal';

// ── Types ─────────────────────────────────────────────────────────────────────

interface Props {
  projectId: string;
  projectName: string;
  isOwner: boolean;
}

// ── Sub-components ────────────────────────────────────────────────────────────

function RemoveConfirm({ repo, onConfirm, onCancel, loading }: {
  repo: RepoSummary; onConfirm: () => void; onCancel: () => void; loading: boolean;
}) {
  return (
    <div className="mt-2 p-3 has-background-warning-light" style={{ borderRadius: 4 }}>
      <p className="is-size-7">
        Remove <strong>{repo.name}</strong>? Existing drafts on disk are not deleted, but no new drafts will be detected until the repo is added again.
      </p>
      <div className="is-flex mt-2" style={{ gap: '0.5rem' }}>
        <button className="button is-small is-danger" onClick={onConfirm} disabled={loading}>
          Confirm remove
        </button>
        <button className="button is-small" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}

function RepoRow({ repo, isOwner, onRemoveStart, onConfigureStart }: {
  repo: RepoSummary; isOwner: boolean;
  onRemoveStart: (_id: string) => void;
  onConfigureStart: (_id: string) => void;
}) {
  return (
    <div className="is-flex is-align-items-center py-2" style={{ gap: '0.75rem', borderBottom: '1px solid var(--bulma-border-weak)' }}>
      <span className="is-size-7" style={{ flex: 1 }}>{repo.name}</span>
      <span className="is-size-7 has-text-grey">{repo.path}</span>
      {isOwner && (
        <>
          <button className="button is-small is-ghost" onClick={() => onConfigureStart(repo.id)}>
            Configure
          </button>
          <button className="button is-small is-ghost has-text-danger" onClick={() => onRemoveStart(repo.id)}>
            Remove
          </button>
        </>
      )}
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function RepositoriesBlock({ projectId, projectName, isOwner }: Props) {
  const { repos, refresh } = useProjectRepos(projectId);
  const [pendingRemoveId, setPendingRemoveId] = useState<string | null>(null);
  const [removeLoading, setRemoveLoading] = useState(false);
  const [showAddModal, setShowAddModal] = useState(false);
  const [configureRepoId, setConfigureRepoId] = useState<string | null>(null);
  const [globalProvider, setGlobalProvider] = useState<string | null>(null);

  useEffect(() => {
    invoke<{ provider: string; connected: boolean }[]>('list_scheduler_profiles', { projectId })
      .then((profiles) => {
        const connected = profiles.find((p) => p.connected);
        setGlobalProvider(connected?.provider ?? null);
      })
      .catch(() => { /* non-critical — modal will show NoProviderView */ });
  }, [projectId]);

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

  const pendingRepo = repos.find((r) => r.id === pendingRemoveId) ?? null;

  return (
    <div>
      <p className="is-size-6 has-text-weight-medium mb-3">Repositories</p>
      {repos.length === 0 && (
        <p className="is-size-7 has-text-grey mb-3">
          {isOwner
            ? 'No repositories connected. Add one to start detecting drafts.'
            : 'No repositories connected. Ask a workspace owner to add a repository.'}
        </p>
      )}
      {repos.map((repo) => (
        <div key={repo.id}>
          <RepoRow repo={repo} isOwner={isOwner} onRemoveStart={setPendingRemoveId} onConfigureStart={setConfigureRepoId} />
          {pendingRemoveId === repo.id && pendingRepo && (
            <RemoveConfirm repo={pendingRepo} onConfirm={handleConfirmRemove}
              onCancel={() => setPendingRemoveId(null)} loading={removeLoading} />
          )}
        </div>
      ))}
      {configureRepoId && (() => {
        const repo = repos.find((r) => r.id === configureRepoId);
        return repo ? (
          <RepoConfigureModal
            repoId={repo.id}
            repoName={repo.name}
            projectId={projectId}
            currentProvider={globalProvider}
            isOwner={isOwner}
            onClose={() => setConfigureRepoId(null)}
          />
        ) : null;
      })()}
      {isOwner && (
        <button className="button is-small is-light mt-3" onClick={() => setShowAddModal(true)}>
          Add repository
        </button>
      )}
      {showAddModal && (
        <AddRepoModal projectId={projectId} projectName={projectName} onClose={() => { setShowAddModal(false); refresh(); }} />
      )}
    </div>
  );
}
