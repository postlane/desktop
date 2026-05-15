// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faArrowUpRightFromSquare } from '@fortawesome/free-solid-svg-icons';

// ── Types & constants ─────────────────────────────────────────────────────────

const ALL_PROVIDERS = ['zernio', 'upload_post', 'buffer', 'publer', 'outstand', 'webhook'] as const;
type Provider = typeof ALL_PROVIDERS[number];

interface Props {
  projectId: string;
  isOwner: boolean;
}

function providerLabel(provider: string): string {
  const labels: Partial<Record<string, string>> = {
    zernio: 'Zernio',
    upload_post: 'Upload Post',
    buffer: 'Buffer',
    publer: 'Publer',
    outstand: 'Outstand',
    webhook: 'Webhook',
  };
  return labels[provider] ?? (provider.charAt(0).toUpperCase() + provider.slice(1));
}

function providerUrl(provider: string): string | null {
  const urls: Partial<Record<string, string>> = {
    zernio: 'https://zernio.io',
    upload_post: 'https://upload-post.com',
    buffer: 'https://buffer.com',
    publer: 'https://publer.io',
    outstand: 'https://outstand.io',
  };
  return urls[provider] ?? null;
}

// ── ProviderLink ──────────────────────────────────────────────────────────────

function ProviderLink({ provider }: { provider: string }) {
  const url = providerUrl(provider);
  if (!url) return null;
  return (
    <button
      className="button is-small is-ghost"
      style={{ padding: '0 0.25rem', color: 'var(--bulma-grey)' }}
      onClick={() => openUrl(url)}
      aria-label={`Open ${providerLabel(provider)} website`}
    >
      <FontAwesomeIcon icon={faArrowUpRightFromSquare} size="xs" />
    </button>
  );
}

// ── ConnectForm ───────────────────────────────────────────────────────────────

function ConnectForm({ provider, onConnected, onCancel }: {
  provider: string;
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
      await invoke('save_scheduler_credential', { provider, apiKey, repoId: null });
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

// ── ConnectedRow ──────────────────────────────────────────────────────────────

function ConnectedRow({ provider, isOwner, expanded, onExpand, onRekeyed, onCancel, onDisconnect, disconnecting }: {
  provider: string;
  isOwner: boolean;
  expanded: boolean;
  onExpand: () => void;
  onRekeyed: () => void;
  onCancel: () => void;
  onDisconnect: () => void;
  disconnecting: boolean;
}) {
  return (
    <div>
      <div className="is-flex is-align-items-center py-2"
        style={{ gap: '0.75rem', borderBottom: expanded ? 'none' : '1px solid var(--bulma-border-weak)' }}>
        <span className="is-size-7" style={{ flex: 1 }}>{providerLabel(provider)}</span>
        <ProviderLink provider={provider} />
        {isOwner && !expanded && (
          <>
            <button className="button is-small is-ghost" onClick={onExpand}>Change key</button>
            <button className="button is-small is-ghost has-text-danger" onClick={onDisconnect} disabled={disconnecting}>
              Disconnect
            </button>
          </>
        )}
      </div>
      {expanded && (
        <ConnectForm provider={provider} onConnected={onRekeyed} onCancel={onCancel} />
      )}
    </div>
  );
}

// ── AvailableRow ──────────────────────────────────────────────────────────────

function AvailableRow({ provider, expanded, onExpand, onConnected, onCancel }: {
  provider: string;
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
        <ProviderLink provider={provider} />
        {!expanded && (
          <button className="button is-small is-light" onClick={onExpand}>Connect</button>
        )}
      </div>
      {expanded && (
        <ConnectForm provider={provider} onConnected={onConnected} onCancel={onCancel} />
      )}
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function SchedulerBlock({ projectId: _projectId, isOwner }: Props) {
  const [connected, setConnected] = useState<string[]>([]);
  const [expandedProvider, setExpandedProvider] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const loadProfiles = useCallback(() => {
    invoke<string[]>('list_connected_providers', { repoId: null })
      .then(setConnected)
      .catch(() => setConnected([]));
  }, []);

  useEffect(() => { loadProfiles(); }, [loadProfiles]);

  async function handleDisconnect(provider: string) {
    setLoading(true);
    try {
      await invoke('delete_scheduler_credential', { provider, repoId: null });
      loadProfiles();
    } finally {
      setLoading(false);
    }
  }

  const available = ALL_PROVIDERS.filter((p: Provider) => !connected.includes(p));

  return (
    <div>
      <p className="is-size-6 has-text-weight-medium mb-3">Scheduler</p>
      {connected.length === 0 && (
        <p className="is-size-7 has-text-grey mb-2">No scheduler connected.</p>
      )}
      {connected.map((p) => (
        <ConnectedRow key={p} provider={p} isOwner={isOwner}
          expanded={expandedProvider === p}
          onExpand={() => setExpandedProvider(p)}
          onRekeyed={() => { setExpandedProvider(null); loadProfiles(); }}
          onCancel={() => setExpandedProvider(null)}
          onDisconnect={() => handleDisconnect(p)}
          disconnecting={loading}
        />
      ))}
      {isOwner && available.map((p) => (
        <AvailableRow key={p} provider={p}
          expanded={expandedProvider === p}
          onExpand={() => setExpandedProvider(p)}
          onConnected={() => { setExpandedProvider(null); loadProfiles(); }}
          onCancel={() => setExpandedProvider(null)}
        />
      ))}
    </div>
  );
}
