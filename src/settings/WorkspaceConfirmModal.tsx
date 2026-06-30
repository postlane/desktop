// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '../ipc/invoke';
import { useAsyncCommand } from '../hooks/useAsyncCommand';

export interface DiscoveredRepo {
  name: string;
  path: string;
  posts_dir: string;
}

export interface WorkspaceSetupResult {
  workspace_id: string;
  workspace_path: string;
  discovered_repos: DiscoveredRepo[];
}

interface Props {
  result: WorkspaceSetupResult;
  onConfirm: (selectedPaths: string[]) => void;
  onCancel: () => void;
}

function RepoCheckbox({ repo, checked, onChange }: {
  repo: DiscoveredRepo; checked: boolean; onChange: (checked: boolean) => void;
}) {
  return (
    <label className="is-flex" style={{ gap: '0.5rem', alignItems: 'center', cursor: 'pointer' }}>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span className="is-size-7 has-text-weight-medium">{repo.name}</span>
      <span className="is-size-7 has-text-grey">{repo.path}</span>
    </label>
  );
}

function useConfirmWorkspace(result: WorkspaceSetupResult, onConfirm: (paths: string[]) => void) {
  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(result.discovered_repos.map((r) => r.path))
  );
  const { loading, error, run } = useAsyncCommand();

  function toggle(path: string, checked: boolean) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (checked) next.add(path); else next.delete(path);
      return next;
    });
  }

  async function handleConfirm() {
    const paths = Array.from(selected);
    const r = await run(async () => { await invoke('confirm_workspace_repos', { workspaceId: result.workspace_id, selectedPaths: paths }); return true; });
    if (r !== null) {
      onConfirm(paths);
    }
  }

  return { selected, loading, error, toggle, handleConfirm };
}

export default function WorkspaceConfirmModal({ result, onConfirm, onCancel }: Props) {
  const { selected, loading, error, toggle, handleConfirm } = useConfirmWorkspace(result, onConfirm);
  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onCancel} />
      <div className="modal-card" role="dialog" aria-modal="true">
        <header className="modal-card-head" style={{ borderBottom: 'none', backgroundColor: 'white' }}>
          <p className="modal-card-title is-size-6">Confirm repositories</p>
          <button className="delete" onClick={onCancel} aria-label="Close" />
        </header>
        <section className="modal-card-body">
          <p className="is-size-7 has-text-grey mb-3">Select the repositories to include in this workspace.</p>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
            {result.discovered_repos.map((repo) => (
              <RepoCheckbox key={repo.path} repo={repo} checked={selected.has(repo.path)}
                onChange={(checked) => toggle(repo.path, checked)} />
            ))}
          </div>
          {error && <p role="alert" className="is-size-7 has-text-danger mt-3">{error}</p>}
        </section>
        <footer className="modal-card-foot is-justify-content-flex-end"
          style={{ gap: '0.5rem', borderTop: 'none', backgroundColor: 'white' }}>
          <button className="button is-ghost" onClick={onCancel}>Cancel</button>
          <button className="button is-primary" onClick={handleConfirm}
            disabled={selected.size === 0 || loading}>
            {loading ? 'Adding…' : 'Confirm'}
          </button>
        </footer>
      </div>
    </div>
  );
}
