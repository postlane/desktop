// SPDX-License-Identifier: BUSL-1.1

import type { FieldConflict } from './MigrationBanner';

interface Props {
  repoName: string;
  conflicts: FieldConflict[];
  onConfirm: () => void;
  onCancel: () => void;
}

export default function ConflictDiffView({ repoName, conflicts, onConfirm, onCancel }: Props) {
  return (
    <div className="box">
      <p className="is-size-6 has-text-weight-medium mb-3">
        Config conflicts in <strong>{repoName}</strong>
      </p>
      <p className="is-size-7 has-text-grey mb-3">
        The workspace config will win. These fields will be overwritten.
      </p>
      <table className="table is-fullwidth is-size-7 mb-3">
        <thead>
          <tr>
            <th>Field</th>
            <th>Repository value</th>
            <th>Workspace value (wins)</th>
          </tr>
        </thead>
        <tbody>
          {conflicts.map((c) => (
            <tr key={c.field_key}>
              <td>{c.label}</td>
              <td className="has-text-grey">{c.repo_value}</td>
              <td className="has-text-success">{c.workspace_value}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        <button className="button is-small is-primary" onClick={onConfirm} aria-label="Confirm and migrate">
          Confirm and migrate
        </button>
        <button className="button is-small is-light" onClick={onCancel} aria-label="Cancel">
          Cancel
        </button>
      </div>
    </div>
  );
}
