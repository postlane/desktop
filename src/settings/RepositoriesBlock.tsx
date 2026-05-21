// SPDX-License-Identifier: BUSL-1.1

// Deduplication policy (2026-05-21): a repo present in both the GitHub App list and
// the folder-connected list is shown once, in the GitHub App section, with a
// "local folder linked" indicator. It is suppressed from the folder section entirely.
// The folder connection remains in the data model; deduplication is display-only.

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { useProjectRepos } from '../hooks/useRepoData';
import type { RepoSummary } from '../hooks/useRepoData';
import AddRepoModal from '../wizard/AddRepoModal';

// ── Types ─────────────────────────────────────────────────────────────────────

interface Props {
  projectId: string;
  projectName: string;
  isOwner: boolean;
}

interface GitHubAppRepo {
  id: number;
  name: string;
  full_name: string;
  private: boolean;
  html_url: string;
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

function RepoRow({ repo, isOwner, onRemoveStart }: {
  repo: RepoSummary; isOwner: boolean;
  onRemoveStart: (_id: string) => void;
}) {
  return (
    <div className="is-flex is-align-items-center py-2" style={{ gap: '0.75rem', borderBottom: '1px solid var(--bulma-border-weak)' }}>
      <span className="is-size-7" style={{ flex: 1 }}>{repo.name}</span>
      <span className="is-size-7 has-text-grey">{repo.path}</span>
      {isOwner && (
        <button className="button is-small is-ghost has-text-danger" onClick={() => onRemoveStart(repo.id)}>
          Remove
        </button>
      )}
    </div>
  );
}

function GitHubAppRepoRow({ repo, folderLinked }: { repo: GitHubAppRepo; folderLinked: boolean }) {
  return (
    <div className="is-flex is-align-items-center py-2" style={{ gap: '0.75rem', borderBottom: '1px solid var(--bulma-border-weak)' }}>
      <span className="is-size-7" style={{ flex: 1 }}>
        {repo.name}
        <span className="has-text-grey ml-2">{repo.full_name}</span>
      </span>
      {folderLinked && <span className="is-size-7 has-text-grey">local folder linked</span>}
      <a className="is-size-7 has-text-grey" href={repo.html_url} target="_blank" rel="noopener noreferrer">
        {repo.html_url}
      </a>
    </div>
  );
}

// ── State hooks ───────────────────────────────────────────────────────────────

function useRepoActions(refresh: () => void) {
  const [pendingRemoveId, setPendingRemoveId] = useState<string | null>(null);
  const [removeLoading, setRemoveLoading] = useState(false);
  const [showAddModal, setShowAddModal] = useState(false);

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

  return {
    pendingRemoveId, setPendingRemoveId, removeLoading,
    showAddModal, setShowAddModal,
    handleConfirmRemove,
  };
}

function useGitHubAppRepos(projectId: string) {
  const [appRepos, setAppRepos] = useState<GitHubAppRepo[]>([]);

  useEffect(() => {
    invoke<GitHubAppRepo[]>('list_github_app_repos', { projectId })
      .then((repos) => setAppRepos(Array.isArray(repos) ? repos : []))
      .catch(() => {});
  }, [projectId]);

  return appRepos;
}

// ── Main component ────────────────────────────────────────────────────────────

export default function RepositoriesBlock({ projectId, projectName, isOwner }: Props) {
  const { repos, refresh } = useProjectRepos(projectId);
  const appRepos = useGitHubAppRepos(projectId);

  const {
    pendingRemoveId, setPendingRemoveId, removeLoading,
    showAddModal, setShowAddModal,
    handleConfirmRemove,
  } = useRepoActions(refresh);

  const appRepoNames = new Set(appRepos.map((r) => r.name));
  const folderOnlyRepos = repos.filter((r) => !appRepoNames.has(r.name));
  const pendingRepo = repos.find((r) => r.id === pendingRemoveId) ?? null;
  const folderLinkedNames = new Set(repos.map((r) => r.name));

  return (
    <div>
      <p className="is-size-6 has-text-weight-medium mb-3">Repositories</p>

      {appRepos.length > 0 && (
        <div className="mb-4">
          <p className="is-size-7 has-text-weight-semibold has-text-grey mb-2">GitHub App</p>
          {appRepos.map((repo) => (
            <GitHubAppRepoRow key={repo.id} repo={repo} folderLinked={folderLinkedNames.has(repo.name)} />
          ))}
        </div>
      )}

      {folderOnlyRepos.length === 0 && appRepos.length === 0 && (
        <p className="is-size-7 has-text-grey mb-3">
          {isOwner
            ? 'No repositories connected. Add one to start detecting drafts.'
            : 'No repositories connected. Ask a workspace owner to add a repository.'}
        </p>
      )}

      {folderOnlyRepos.map((repo) => (
        <div key={repo.id}>
          <RepoRow repo={repo} isOwner={isOwner} onRemoveStart={setPendingRemoveId} />
          {pendingRemoveId === repo.id && pendingRepo && (
            <RemoveConfirm repo={pendingRepo} onConfirm={handleConfirmRemove}
              onCancel={() => setPendingRemoveId(null)} loading={removeLoading} />
          )}
        </div>
      ))}

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
