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

function ConnectForm({ provider, repoId, onConnected, onCancel }: {
  provider: string;
  repoId: string;
  onConnected: () => void;
  onCancel: () => void;
}) {
  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [connectedNames, setConnectedNames] = useState<Record<string, string> | null>(null);

  async function handleConnect() {
    setLoading(true);
    setError(null);
    try {
      const names = await invoke<Record<string, string>>('save_scheduler_credential', { provider, apiKey, repoId });
      setConnectedNames(names ?? {});
      setTimeout(() => onConnected(), 5000);
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
      {connectedNames !== null && (
        <p role="status" className="is-size-7 has-text-success mt-1">
          {(() => {
            const unique = [...new Set(Object.values(connectedNames))];
            return unique.length > 0 ? `Connected as ${unique.join(', ')}` : 'Connected';
          })()}
        </p>
      )}
      {error && <p role="alert" className="is-size-7 has-text-danger mt-1">{error}</p>}
    </div>
  );
}

// ── ConnectedRow ──────────────────────────────────────────────────────────────

function ConnectedRow({ provider, repoId, isOwner, expanded, accountNames, onExpand, onRekeyed, onCancel, onDisconnect, disconnecting }: {
  provider: string;
  repoId: string;
  isOwner: boolean;
  expanded: boolean;
  accountNames: Record<string, string>;
  onExpand: () => void;
  onRekeyed: () => void;
  onCancel: () => void;
  onDisconnect: () => void;
  disconnecting: boolean;
}) {
  const names = Object.values(accountNames);
  return (
    <div>
      <div className="is-flex is-align-items-center py-2"
        style={{ gap: '0.75rem', borderBottom: expanded ? 'none' : '1px solid var(--bulma-border-weak)' }}>
        <div style={{ flex: 1 }}>
          <span className="is-size-7">{providerLabel(provider)}</span>
          {names.length > 0 && (
            <span className="ml-2">
              {names.map((name) => (
                <span key={name} className="tag is-light is-small mr-1">{name}</span>
              ))}
            </span>
          )}
        </div>
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
        <ConnectForm provider={provider} repoId={repoId} onConnected={onRekeyed} onCancel={onCancel} />
      )}
    </div>
  );
}

// ── AvailableRow ──────────────────────────────────────────────────────────────

function AvailableRow({ provider, repoId, expanded, onExpand, onConnected, onCancel }: {
  provider: string;
  repoId: string;
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
        <ConnectForm provider={provider} repoId={repoId} onConnected={onConnected} onCancel={onCancel} />
      )}
    </div>
  );
}

// ── Module-level action helpers ───────────────────────────────────────────────

async function disconnectProvider(
  provider: string, projectId: string,
  setLoading: (v: boolean) => void, loadProfiles: () => void,
): Promise<void> {
  setLoading(true);
  try {
    await invoke('delete_scheduler_credential', { provider, repoId: projectId });
    loadProfiles();
  } finally {
    setLoading(false);
  }
}

async function syncAccounts(
  projectId: string,
  setLoading: (v: boolean) => void,
  setSyncStatus: (s: { ok: boolean; message: string } | null) => void,
  loadProfiles: () => void,
): Promise<void> {
  setLoading(true);
  setSyncStatus(null);
  try {
    const result = await invoke<{ providers_synced: string[]; errors: string[] }>(
      'refresh_scheduler_accounts', { repoId: projectId }
    );
    if (result.errors.length > 0) {
      setSyncStatus({ ok: false, message: result.errors.join('; ') });
    } else {
      setSyncStatus({ ok: true, message: `Synced ${result.providers_synced.join(', ')}` });
      loadProfiles();
    }
  } catch (e: unknown) {
    setSyncStatus({ ok: false, message: String(e) });
  } finally {
    setLoading(false);
  }
}

// ── Main component ────────────────────────────────────────────────────────────

export default function SchedulerBlock({ projectId, isOwner }: Props) {
  const [connected, setConnected] = useState<string[]>([]);
  const [accountNames, setAccountNames] = useState<Record<string, string>>({});
  const [expandedProvider, setExpandedProvider] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [syncStatus, setSyncStatus] = useState<{ ok: boolean; message: string } | null>(null);

  const loadProfiles = useCallback(() => {
    invoke<string[]>('list_connected_providers', { repoId: projectId }).then(setConnected).catch(() => setConnected([]));
    invoke<Record<string, string>>('get_scheduler_account_names', { repoId: projectId }).then(setAccountNames).catch(() => setAccountNames({}));
  }, [projectId]);

  useEffect(() => { loadProfiles(); }, [loadProfiles]);

  const available = ALL_PROVIDERS.filter((p: Provider) => !connected.includes(p));

  return (
    <div>
      <div className="is-flex is-align-items-center mb-3">
        <p className="is-size-6 has-text-weight-medium" style={{ flex: 1 }}>Scheduler</p>
        {connected.length > 0 && (
          <button className="button is-small is-ghost" onClick={() => syncAccounts(projectId, setLoading, setSyncStatus, loadProfiles)} disabled={loading} aria-label="Sync accounts">
            Sync accounts
          </button>
        )}
      </div>
      {syncStatus && (
        <p
          role="alert"
          className={`is-size-7 mb-2 ${syncStatus.ok ? 'has-text-success' : 'has-text-danger'}`}
        >
          {syncStatus.message}
        </p>
      )}
      {connected.length === 0 && (
        <p className="is-size-7 has-text-grey mb-2">No scheduler connected.</p>
      )}
      {connected.map((p) => (
        <ConnectedRow key={p} provider={p} repoId={projectId} isOwner={isOwner}
          accountNames={accountNames}
          expanded={expandedProvider === p}
          onExpand={() => setExpandedProvider(p)}
          onRekeyed={() => { setExpandedProvider(null); loadProfiles(); }}
          onCancel={() => setExpandedProvider(null)}
          onDisconnect={() => disconnectProvider(p, projectId, setLoading, loadProfiles)}
          disconnecting={loading}
        />
      ))}
      {isOwner && available.map((p) => (
        <AvailableRow key={p} provider={p} repoId={projectId}
          expanded={expandedProvider === p}
          onExpand={() => setExpandedProvider(p)}
          onConnected={() => { setExpandedProvider(null); loadProfiles(); }}
          onCancel={() => setExpandedProvider(null)}
        />
      ))}
    </div>
  );
}
