// SPDX-License-Identifier: BUSL-1.1

import type { MigrationResult } from './MigrationBanner';

interface Props {
  result: MigrationResult;
  workspacePath: string;
  onRetry: (failedPaths: string[]) => void;
}

function statusTag(status: MigrationResult['results'][0]['status']): string {
  return status.tag;
}

export default function MigrationResultView({ result, workspacePath: _workspacePath, onRetry }: Props) {
  const successes = result.results.filter((r) => statusTag(r.status) === 'success');
  const failures = result.results.filter((r) => statusTag(r.status) === 'verification_failed');
  const mismatches = result.results.filter((r) => statusTag(r.status) === 'project_id_mismatch');

  function handleRetry() {
    const failedPaths = failures.map((r) => r.repo_path);
    onRetry(failedPaths);
  }

  return (
    <div className="box">
      {successes.length > 0 && (
        <p className="has-text-success is-size-7 mb-2">
          {successes.length} repositor{successes.length === 1 ? 'y' : 'ies'} migrated successfully.
        </p>
      )}

      {failures.map((r) => (
        <div key={r.repo_path} className="mb-2">
          <p className="is-size-7 has-text-danger">
            <strong>{r.repo_name}</strong>
            {' — '}
            {r.status.tag === 'verification_failed' ? r.status.error : ''}
          </p>
        </div>
      ))}

      {mismatches.map((r) => (
        <p key={r.repo_path} className="is-size-7 has-text-warning mb-1">
          <strong>{r.repo_name}</strong> — belongs to a different project; not migrated.
        </p>
      ))}

      {failures.length > 0 && (
        <button className="button is-small is-warning mt-2" onClick={handleRetry} aria-label="Retry">
          Retry failed repos
        </button>
      )}
    </div>
  );
}
