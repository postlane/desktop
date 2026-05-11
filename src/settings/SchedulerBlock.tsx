// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '../ipc/invoke';

// ── Types ─────────────────────────────────────────────────────────────────────

interface SchedulerProviderStatus {
  provider: string;
  connected: boolean;
}

interface Props {
  projectId: string;
  isOwner: boolean;
}

function providerLabel(provider: string): string {
  if (provider === 'zernio') return 'Zernio';
  return provider.charAt(0).toUpperCase() + provider.slice(1);
}

// ── ConnectForm ───────────────────────────────────────────────────────────────

function ConnectForm({ provider, projectId, onConnected, onCancel }: {
  provider: string;
  projectId: string;
  onConnected: () => void;
  onCancel: () => void;
}) {
  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleConnect() {
    setLoading(true);
    setError(null);
    try {
      await invoke('add_scheduler_credential', { provider, apiKey, projectId });
      setApiKey('');
      onConnected();
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="mt-2 pb-2" style={{ borderBottom: '1px solid var(--bulma-border-weak)' }}>
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        <label className="is-sr-only" htmlFor={`scheduler-api-key-${provider}`}>API key</label>
        <input id={`scheduler-api-key-${provider}`} aria-label="API key"
          type={showKey ? 'text' : 'password'}
          className="input is-small" value={apiKey} onChange={(e) => setApiKey(e.target.value)}
          placeholder={`Enter your ${providerLabel(provider)} API key`} style={{ flex: 1 }} />
        <button className="button is-small is-ghost" onClick={() => setShowKey((v) => !v)}>
          {showKey ? 'Hide' : 'Show'}
        </button>
        <button className="button is-small is-primary" onClick={handleConnect} disabled={loading || !apiKey}>
          Connect
        </button>
        <button className="button is-small is-ghost" onClick={onCancel} disabled={loading}>
          Cancel
        </button>
      </div>
      {error && <p role="alert" className="is-size-7 has-text-danger mt-1">{error}</p>}
    </div>
  );
}

// ── AvailableRow ──────────────────────────────────────────────────────────────

function AvailableRow({ provider, projectId, expanded, onExpand, onConnected, onCancel }: {
  provider: string;
  projectId: string;
  expanded: boolean;
  onExpand: () => void;
  onConnected: () => void;
  onCancel: () => void;
}) {
  return (
    <div>
      <div className="is-flex is-align-items-center py-2"
        style={{ gap: '0.75rem', borderBottom: expanded ? 'none' : '1px solid var(--bulma-border-weak)' }}>
        <span className="is-size-7 has-text-grey" style={{ flex: 1 }}>{providerLabel(provider)}</span>
        {!expanded && (
          <button className="button is-small is-outlined" onClick={onExpand}>Connect</button>
        )}
      </div>
      {expanded && (
        <ConnectForm provider={provider} projectId={projectId} onConnected={onConnected} onCancel={onCancel} />
      )}
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function SchedulerBlock({ projectId, isOwner }: Props) {
  const [profiles, setProfiles] = useState<SchedulerProviderStatus[]>([]);
  const [expandedProvider, setExpandedProvider] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const loadProfiles = useCallback(() => {
    invoke<SchedulerProviderStatus[]>('list_scheduler_profiles', { projectId })
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

  const connected = profiles.filter((p) => p.connected);
  const available = profiles.filter((p) => !p.connected);

  return (
    <div>
      <p className="is-size-6 has-text-weight-medium mb-3">Scheduler</p>
      {profiles.length === 0 && <p className="is-size-7 has-text-grey">No scheduler connected.</p>}
      {connected.map((p) => (
        <div key={p.provider} className="is-flex is-align-items-center py-2"
          style={{ gap: '0.75rem', borderBottom: '1px solid var(--bulma-border-weak)' }}>
          <span className="is-size-7" style={{ flex: 1 }}>{providerLabel(p.provider)}</span>
          {isOwner && (
            <button className="button is-small is-ghost has-text-danger"
              onClick={() => handleDisconnect(p.provider)} disabled={loading}>
              Disconnect
            </button>
          )}
        </div>
      ))}
      {isOwner && available.map((p) => (
        <AvailableRow key={p.provider} provider={p.provider} projectId={projectId}
          expanded={expandedProvider === p.provider}
          onExpand={() => setExpandedProvider(p.provider)}
          onConnected={() => { setExpandedProvider(null); loadProfiles(); }}
          onCancel={() => setExpandedProvider(null)}
        />
      ))}
    </div>
  );
}
