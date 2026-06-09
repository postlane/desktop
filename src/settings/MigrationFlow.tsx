// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import ConflictDiffView from './ConflictDiffView';
import MigrationResultView from './MigrationResultView';
import type { RepoConflicts, MigrationResult } from './MigrationBanner';

// ── State machine ─────────────────────────────────────────────────────────────

type FlowState =
  | { tag: 'loading' }
  | { tag: 'no_workspace' }
  | { tag: 'conflicts'; wsPath: string; conflicts: RepoConflicts[]; idx: number }
  | { tag: 'confirm'; wsPath: string }
  | { tag: 'running' }
  | { tag: 'result'; wsPath: string; result: MigrationResult };

interface Props {
  projectId: string;
  onDone: () => void;
}

export default function MigrationFlow({ projectId, onDone }: Props) {
  const [state, setState] = useState<FlowState>({ tag: 'loading' });

  useEffect(() => { void load(projectId, setState); }, [projectId]);

  if (state.tag === 'loading') {
    return <p className="is-size-7 has-text-grey">Loading migration details…</p>;
  }
  if (state.tag === 'no_workspace') {
    return (
      <div className="box">
        <p className="is-size-7 has-text-danger mb-2">
          No workspace found for this project. Register a workspace first.
        </p>
        <button className="button is-small is-light" onClick={onDone}>Cancel</button>
      </div>
    );
  }
  if (state.tag === 'conflicts') {
    return (
      <ConflictStep
        wsPath={state.wsPath}
        conflicts={state.conflicts}
        idx={state.idx}
        onAdvance={(nextIdx) => setState({ ...state, idx: nextIdx })}
        onMigrate={() => void runMigration(state.wsPath, setState)}
        onCancel={onDone}
      />
    );
  }
  if (state.tag === 'confirm') {
    return (
      <ConfirmView
        onConfirm={() => void runMigration(state.wsPath, setState)}
        onCancel={onDone}
      />
    );
  }
  if (state.tag === 'running') {
    return <p className="is-size-7 has-text-grey">Migrating…</p>;
  }
  return (
    <div>
      <MigrationResultView
        result={state.result}
        workspacePath={state.wsPath}
        onRetry={(failedPaths) => void retryMigration(state.wsPath, failedPaths, setState)}
      />
      <button className="button is-small is-light mt-2" onClick={onDone} aria-label="Done">Done</button>
    </div>
  );
}

// ── Sub-components ────────────────────────────────────────────────────────────

interface ConflictStepProps {
  wsPath: string;
  conflicts: RepoConflicts[];
  idx: number;
  onAdvance: (nextIdx: number) => void;
  onMigrate: () => void;
  onCancel: () => void;
}

function ConflictStep({ conflicts, idx, onAdvance, onMigrate, onCancel }: ConflictStepProps) {
  const current = conflicts[idx];
  return (
    <ConflictDiffView
      repoName={current.repo_name}
      conflicts={current.conflicts}
      onConfirm={() => {
        const nextIdx = idx + 1;
        if (nextIdx >= conflicts.length) { onMigrate(); } else { onAdvance(nextIdx); }
      }}
      onCancel={onCancel}
    />
  );
}

function ConfirmView({ onConfirm, onCancel }: { onConfirm: () => void; onCancel: () => void }) {
  return (
    <div className="box">
      <p className="is-size-6 mb-2">Ready to migrate posts to workspace.</p>
      <p className="is-size-7 has-text-grey mb-3">
        Posts will be copied to the workspace folder. Originals are deleted after byte-count verification passes.
      </p>
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        <button
          className="button is-small is-primary"
          onClick={onConfirm}
          aria-label="Confirm and migrate"
        >
          Confirm and migrate
        </button>
        <button className="button is-small is-light" onClick={onCancel} aria-label="Cancel">
          Cancel
        </button>
      </div>
    </div>
  );
}

// ── Side-effect helpers ───────────────────────────────────────────────────────

async function load(projectId: string, setState: (s: FlowState) => void) {
  try {
    const wsPath = await invoke<string | null>('get_workspace_path', { projectId });
    if (!wsPath) { setState({ tag: 'no_workspace' }); return; }
    const conflicts = await invoke<RepoConflicts[]>('get_migration_conflicts', { workspacePath: wsPath });
    setState(conflicts.length > 0
      ? { tag: 'conflicts', wsPath, conflicts, idx: 0 }
      : { tag: 'confirm', wsPath });
  } catch {
    setState({ tag: 'no_workspace' });
  }
}

async function runMigration(wsPath: string, setState: (s: FlowState) => void) {
  setState({ tag: 'running' });
  try {
    const result = await invoke<MigrationResult>('start_workspace_migration', { workspacePath: wsPath });
    setState({ tag: 'result', wsPath, result });
  } catch {
    setState({ tag: 'no_workspace' });
  }
}

async function retryMigration(wsPath: string, failedPaths: string[], setState: (s: FlowState) => void) {
  setState({ tag: 'running' });
  try {
    const result = await invoke<MigrationResult>('retry_workspace_migration', {
      workspacePath: wsPath,
      repoPaths: failedPaths,
    });
    setState({ tag: 'result', wsPath, result });
  } catch {
    setState({ tag: 'no_workspace' });
  }
}
