// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

// ── Types (mirror Rust workspace_path_check) ──────────────────────────────────

export interface RenamedCandidate {
  path: string;
  name: string;
  modified_secs: number;
}

export type WorkspacePathStatus =
  | { tag: 'ok' }
  | { tag: 'renamed'; candidates: RenamedCandidate[] }
  | { tag: 'missing' };

export interface WorkspaceCheckResult {
  workspace_id: string;
  workspace_path: string;
  workspace_name: string;
  status: WorkspacePathStatus;
}

// ── Hook ──────────────────────────────────────────────────────────────────────

export function useWorkspaceStatus(projectId: string) {
  const [result, setResult] = useState<WorkspaceCheckResult | null>(null);

  useEffect(() => {
    invoke<WorkspaceCheckResult[]>('check_workspace_paths')
      .then((all) => {
        setResult(all.find((r) => r.workspace_id === projectId) ?? null);
      })
      .catch(() => {});
  }, [projectId]);

  function clearStatus() { setResult(null); }
  return { result, clearStatus };
}

// ── Shared locate-folder logic ────────────────────────────────────────────────

function useLocateFolder(workspaceId: string, onResolved: () => void) {
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function locate() {
    const selected = await openDialog({ directory: true });
    if (typeof selected !== 'string') return;
    setLoading(true);
    setError(null);
    try {
      await invoke('locate_workspace_folder', { workspaceId, folderPath: selected });
      onResolved();
    } catch (e) {
      setError(typeof e === 'string' ? e : 'Failed to locate workspace folder');
    } finally {
      setLoading(false);
    }
  }
  return { locate, loading, error };
}

// ── Rename banner (single candidate) ─────────────────────────────────────────

function SingleRenameBanner({ result, candidate, onResolved }: {
  result: WorkspaceCheckResult;
  candidate: RenamedCandidate;
  onResolved: () => void;
}) {
  const [loading, setLoading] = useState(false);
  const { locate, loading: locLoading, error: locError } = useLocateFolder(result.workspace_id, onResolved);

  async function handleUpdate() {
    setLoading(true);
    try {
      await invoke('update_workspace_path', { workspaceId: result.workspace_id, newPath: candidate.path });
      onResolved();
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="notification is-warning is-light mt-2">
      <p className="is-size-7 has-text-weight-medium mb-1">
        Workspace folder may have been renamed: {result.workspace_name} → {candidate.name}
      </p>
      <div className="is-flex mt-2" style={{ gap: '0.5rem' }}>
        <button className="button is-small is-warning" onClick={handleUpdate} disabled={loading}>
          Update path
        </button>
        <button className="button is-small is-light" onClick={locate} disabled={locLoading}>
          Locate manually
        </button>
      </div>
      {locError && <p className="is-size-7 has-text-danger mt-1">{locError}</p>}
    </div>
  );
}

// ── Rename banner (multiple candidates) ──────────────────────────────────────

function MultipleRenamesBanner({ result, candidates, onResolved }: {
  result: WorkspaceCheckResult;
  candidates: RenamedCandidate[];
  onResolved: () => void;
}) {
  const sorted = [...candidates].sort((a, b) => b.modified_secs - a.modified_secs);
  const [selected, setSelected] = useState(sorted[0]?.path ?? '');
  const [loading, setLoading] = useState(false);
  const { locate, loading: locLoading, error: locError } = useLocateFolder(result.workspace_id, onResolved);

  async function handleUpdate() {
    setLoading(true);
    try {
      await invoke('update_workspace_path', { workspaceId: result.workspace_id, newPath: selected });
      onResolved();
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="notification is-warning is-light mt-2">
      <p className="is-size-7 has-text-weight-medium mb-1">
        Workspace folder may have been renamed: {result.workspace_name}
      </p>
      <select className="select is-small mb-2" value={selected}
        onChange={(e) => setSelected(e.target.value)}>
        {sorted.map((c) => (
          <option key={c.path} value={c.path}>{c.name}</option>
        ))}
      </select>
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        <button className="button is-small is-warning" onClick={handleUpdate} disabled={loading || !selected}>
          Update path
        </button>
        <button className="button is-small is-light" onClick={locate} disabled={locLoading}>
          Locate manually
        </button>
      </div>
      {locError && <p className="is-size-7 has-text-danger mt-1">{locError}</p>}
    </div>
  );
}

// ── Missing banner ────────────────────────────────────────────────────────────

function MissingBanner({ result, onResolved, onDismiss }: {
  result: WorkspaceCheckResult;
  onResolved: () => void;
  onDismiss: () => void;
}) {
  const { locate, loading, error } = useLocateFolder(result.workspace_id, onResolved);

  return (
    <div className="notification is-danger is-light mt-2">
      <p className="is-size-7 has-text-weight-medium mb-1">
        Workspace folder not found: {result.workspace_path}
      </p>
      <div className="is-flex mt-2" style={{ gap: '0.5rem' }}>
        <button className="button is-small is-danger is-light" onClick={locate} disabled={loading}>
          Locate folder
        </button>
        <button className="button is-small is-light" onClick={onDismiss}>
          Dismiss
        </button>
      </div>
      {error && <p className="is-size-7 has-text-danger mt-1">{error}</p>}
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

interface Props {
  result: WorkspaceCheckResult;
  onResolved: () => void;
  onDismiss: () => void;
}

export default function WorkspaceMissingBanner({ result, onResolved, onDismiss }: Props) {
  const { status } = result;
  if (status.tag === 'ok') return null;
  if (status.tag === 'renamed') {
    if (status.candidates.length === 1) {
      return <SingleRenameBanner result={result} candidate={status.candidates[0]} onResolved={onResolved} />;
    }
    return <MultipleRenamesBanner result={result} candidates={status.candidates} onResolved={onResolved} />;
  }
  return <MissingBanner result={result} onResolved={onResolved} onDismiss={onDismiss} />;
}
