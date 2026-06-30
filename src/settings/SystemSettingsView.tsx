// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import type { ModelStatsResponse } from '../types';

// ── Types ─────────────────────────────────────────────────────────────────────

interface WatcherStatus {
  repo_name: string;
  repo_path: string;
  active: boolean;
  last_event_at: string | null;
}

// ── ModelStatsSection ─────────────────────────────────────────────────────────

function ModelStatsSection() {
  const [stats, setStats] = useState<ModelStatsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<ModelStatsResponse>('get_model_stats')
      .then(setStats)
      .catch(() => setError('Could not load draft quality stats.'));
  }, []);

  if (error) return <p className="is-size-7 has-text-danger">{error}</p>;

  if (!stats || stats.total_posts === 0) {
    return <p className="is-size-7 has-text-grey">No data yet — stats appear after your first approved post.</p>;
  }

  const pct = (stats.edit_rate * 100).toFixed(1) + '%';
  return (
    <div className="is-size-7">
      <p><strong>{pct}</strong> edit rate</p>
      <p className="has-text-grey">{stats.edited_posts} edited of {stats.total_posts} posts</p>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function SystemSettingsView() {
  const [version, setVersion] = useState<string | null>(null);
  const [watchers, setWatchers] = useState<WatcherStatus[]>([]);

  useEffect(() => {
    invoke<string>('get_app_version').then(setVersion).catch(console.error);
    invoke<WatcherStatus[]>('get_watcher_status').then(setWatchers).catch(console.error);
  }, []);

  return (
    <div className="px-5 py-4" style={{ maxWidth: '36rem' }}>
      <p className="is-size-5 has-text-weight-semibold mb-5">System</p>
      <div className="field mb-5">
        <label className="label is-small">App version</label>
        <p className="is-size-7">{version ?? '…'}</p>
      </div>
      <div className="field mb-5">
        <label className="label is-small">Watcher health</label>
        {watchers.length === 0 ? (
          <p className="is-size-7 has-text-grey">No repositories configured.</p>
        ) : (
          <table className="table is-narrow is-fullwidth is-size-7">
            <thead><tr><th>Repository</th><th>Status</th></tr></thead>
            <tbody>
              {watchers.map((w) => (
                <tr key={w.repo_path}>
                  <td>{w.repo_name}</td>
                  <td>{w.active ? 'Active' : 'Inactive'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      <div className="field mb-5">
        <label className="label is-small">AI draft quality</label>
        <ModelStatsSection />
      </div>
      <div className="field mb-4">
        <button className="button is-small" disabled title="Coming soon">
          Check for updates
        </button>
      </div>
    </div>
  );
}
