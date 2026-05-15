// SPDX-License-Identifier: BUSL-1.1

export function LoadingView() {
  return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%' }}>
      <p className="is-size-7 has-text-grey">Loading…</p>
    </div>
  );
}

export function QueueLoadError({ error, onRetry }: { error: string; onRetry: () => void }) {
  return (
    <div className="p-5">
      <p className="is-size-7 has-text-danger mb-3">{error}</p>
      <button className="button is-small" onClick={onRetry}>Retry</button>
    </div>
  );
}
