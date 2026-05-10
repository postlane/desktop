// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '../ipc/invoke';

// ── Types ─────────────────────────────────────────────────────────────────────

interface SchedulerProfile {
  provider: string;
  label: string;
}

interface Props {
  projectId: string;
  isOwner: boolean;
}

// ── Sub-components ────────────────────────────────────────────────────────────

function ConnectForm({ projectId, onConnected }: { projectId: string; onConnected: () => void }) {
  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleConnect() {
    setLoading(true);
    setError(null);
    try {
      await invoke('add_scheduler_credential', { provider: 'zernio', apiKey, projectId });
      setApiKey('');
      onConnected();
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="mt-3">
      <label className="label is-small" htmlFor="scheduler-api-key">Zernio API key</label>
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        <input id="scheduler-api-key" aria-label="API key" type={showKey ? 'text' : 'password'}
          className="input is-small" value={apiKey} onChange={(e) => setApiKey(e.target.value)}
          placeholder="Enter your Zernio API key" style={{ flex: 1 }} />
        <button className="button is-small is-ghost" onClick={() => setShowKey((v) => !v)}>
          {showKey ? 'Hide' : 'Show'}
        </button>
        <button className="button is-small is-primary" onClick={handleConnect} disabled={loading || !apiKey}>
          Connect
        </button>
      </div>
      {error && <p role="alert" className="is-size-7 has-text-danger mt-1">{error}</p>}
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function SchedulerBlock({ projectId, isOwner }: Props) {
  const [profiles, setProfiles] = useState<SchedulerProfile[]>([]);
  const [loading, setLoading] = useState(false);

  const loadProfiles = useCallback(() => {
    invoke<SchedulerProfile[]>('list_scheduler_profiles', { projectId })
      .then(setProfiles)
      .catch(() => setProfiles([]));
  }, [projectId]);

  useEffect(() => { loadProfiles(); }, [loadProfiles]);

  async function handleDisconnect(provider: string) {
    setLoading(true);
    try {
      await invoke('remove_scheduler_credential', { provider, projectId });
      loadProfiles();
    } finally {
      setLoading(false);
    }
  }

  return (
    <div>
      <p className="is-size-6 has-text-weight-medium mb-3">Scheduler</p>
      {profiles.length === 0 && (
        <p className="is-size-7 has-text-grey">No scheduler connected.</p>
      )}
      {profiles.map((p) => (
        <div key={p.provider} className="is-flex is-align-items-center py-2"
          style={{ gap: '0.75rem', borderBottom: '1px solid var(--bulma-border-weak)' }}>
          <span className="is-size-7" style={{ flex: 1 }}>{p.label}</span>
          {isOwner && (
            <button className="button is-small is-ghost has-text-danger"
              onClick={() => handleDisconnect(p.provider)} disabled={loading}>
              Disconnect
            </button>
          )}
        </div>
      ))}
      {isOwner && <ConnectForm projectId={projectId} onConnected={loadProfiles} />}
    </div>
  );
}
