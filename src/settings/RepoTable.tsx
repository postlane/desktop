// SPDX-License-Identifier: BUSL-1.1

import { Fragment } from 'react';

export interface RepoConnectionStatus {
  repo_id: string | null;
  github_full_name: string | null;
  local_path: string | null;
  display_name: string;
  github_app_connected: boolean;
  folder_registered: boolean;
  cli_initialized: boolean;
  project_id_mismatch: boolean;
}

export interface RowActions {
  pendingRemoveId: string | null;
  removeLoading: boolean;
  onRemoveStart: (id: string) => void;
  onConfirmRemove: () => void;
  onCancelRemove: () => void;
}

function StatusIcon({ on }: { on: boolean }) {
  return on
    ? <span className="has-text-success">✓</span>
    : <span className="has-text-grey-light">—</span>;
}

function CliIcon({ initialized, mismatch }: { initialized: boolean; mismatch: boolean }) {
  if (initialized && mismatch) {
    return <span className="has-text-warning" title="Initialised for a different project">⚠</span>;
  }
  return <StatusIcon on={initialized} />;
}

function RemoveConfirm({ name, onConfirm, onCancel, loading }: {
  name: string; onConfirm: () => void; onCancel: () => void; loading: boolean;
}) {
  return (
    <div className="p-3 has-background-warning-light" style={{ borderRadius: 4 }}>
      <p className="is-size-7">
        Remove <strong>{name}</strong>? Existing drafts on disk are not deleted,
        but no new drafts will be detected until the repo is added again.
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

function RepoNameCell({ row }: { row: RepoConnectionStatus }) {
  return (
    <td className="is-size-7" style={{ verticalAlign: 'middle' }}>
      <span className="has-text-weight-medium">{row.display_name}</span>
      {row.github_full_name && <span className="has-text-grey ml-2">{row.github_full_name}</span>}
      {row.local_path && (
        <div className="has-text-grey-light" style={{ fontSize: '0.68rem', marginTop: 2 }}>
          {row.local_path}
        </div>
      )}
    </td>
  );
}

function RepoActionsCell({ row, isOwner, isPending, onRemoveStart }: {
  row: RepoConnectionStatus; isOwner: boolean; isPending: boolean;
  onRemoveStart: (id: string) => void;
}) {
  const repoId = row.repo_id;
  return (
    <td className="is-size-7" style={{ verticalAlign: 'middle', whiteSpace: 'nowrap' }}>
      {isOwner && row.folder_registered && repoId && !isPending && (
        <button className="button is-small is-ghost has-text-danger"
          onClick={() => onRemoveStart(repoId)}>Remove</button>
      )}
      {row.github_full_name && (
        <a className="has-text-grey ml-2"
          href={`https://github.com/${row.github_full_name}`}
          target="_blank" rel="noopener noreferrer">↗</a>
      )}
    </td>
  );
}

function RepoStatusRow({ row, isOwner, actions }: {
  row: RepoConnectionStatus; isOwner: boolean; actions: RowActions;
}) {
  const { pendingRemoveId, removeLoading, onRemoveStart, onConfirmRemove, onCancelRemove } = actions;
  const isPending = row.repo_id != null && row.repo_id === pendingRemoveId;
  const center = { verticalAlign: 'middle' as const };
  return (
    <Fragment>
      <tr>
        <RepoNameCell row={row} />
        <td className="has-text-centered is-size-7" style={center}><StatusIcon on={row.github_app_connected} /></td>
        <td className="has-text-centered is-size-7" style={center}><StatusIcon on={row.folder_registered} /></td>
        <td className="has-text-centered is-size-7" style={center}>
          <CliIcon initialized={row.cli_initialized} mismatch={row.project_id_mismatch} />
        </td>
        <RepoActionsCell row={row} isOwner={isOwner} isPending={isPending} onRemoveStart={onRemoveStart} />
      </tr>
      {isPending && (
        <tr>
          <td colSpan={5} style={{ padding: 0 }}>
            <RemoveConfirm name={row.display_name}
              onConfirm={onConfirmRemove} onCancel={onCancelRemove} loading={removeLoading} />
          </td>
        </tr>
      )}
    </Fragment>
  );
}

function ConnectionStatusTable({ rows, isOwner, actions }: {
  rows: RepoConnectionStatus[]; isOwner: boolean; actions: RowActions;
}) {
  return (
    <div style={{ overflowX: 'auto' }}>
      <table className="table is-fullwidth is-narrow mb-0" style={{ tableLayout: 'fixed' }}>
        <colgroup>
          <col style={{ width: '44%' }} /><col style={{ width: '14%' }} />
          <col style={{ width: '14%' }} /><col style={{ width: '10%' }} /><col style={{ width: '18%' }} />
        </colgroup>
        <thead>
          <tr>
            <th className="is-size-7">Repository</th>
            <th className="is-size-7 has-text-centered" title="GitHub App receives push events for this repo">GitHub App</th>
            <th className="is-size-7 has-text-centered" title="Local folder registered in the desktop app">Folder</th>
            <th className="is-size-7 has-text-centered" title="postlane init has been run">CLI</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {rows.map((row, i) => (
            <RepoStatusRow key={row.repo_id ?? row.github_full_name ?? i}
              row={row} isOwner={isOwner} actions={actions} />
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function RepoListSection({ loading, rows, isOwner, actions }: {
  loading: boolean; rows: RepoConnectionStatus[]; isOwner: boolean; actions: RowActions;
}) {
  if (loading) return <p className="is-size-7 has-text-grey">Loading…</p>;
  if (rows.length === 0) {
    return (
      <p className="is-size-7 has-text-grey mb-3">
        {isOwner
          ? 'No repositories connected. Add one to start detecting drafts.'
          : 'No repositories connected. Ask a workspace owner to add a repository.'}
      </p>
    );
  }
  return <ConnectionStatusTable rows={rows} isOwner={isOwner} actions={actions} />;
}
