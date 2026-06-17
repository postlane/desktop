// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback, useRef, type MutableRefObject } from 'react';
import { useAsyncCommand } from '../hooks/useAsyncCommand';
import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faArrowUpRightFromSquare } from '@fortawesome/free-solid-svg-icons';

// ── Types & constants ─────────────────────────────────────────────────────────

const ALL_PROVIDERS = ['zernio', 'upload_post', 'buffer', 'publer', 'outstand', 'webhook'] as const;
type Provider = typeof ALL_PROVIDERS[number];

/** Mirrors scheduler_credentials::SaveCredentialResponse */
interface SaveCredentialResponse {
  account_names: Record<string, string>;
  sync_warning: string | null;
}

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

function formatSuccessMessage(provider: string, names: Record<string, string>): string {
  if (provider === 'upload_post') {
    const user = names['upload_post'] ?? '';
    const channels = [...new Set(Object.entries(names).filter(([k]) => k !== 'upload_post').map(([, v]) => v))];
    if (channels.length > 0) return `Connected as the user account ${user} with the channel ${channels.join(', ')}`;
    return user ? `Connected as the user account ${user}` : 'Connected';
  }
  const unique = [...new Set(Object.values(names))];
  return unique.length > 0 ? `Connected as ${unique.join(', ')}` : 'Connected';
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

// ── UploadPostUsernameField ───────────────────────────────────────────────────

function UploadPostUsernameField({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <div>
      <label className="is-sr-only" htmlFor="scheduler-username-upload_post">Upload Post username</label>
      <input
        id="scheduler-username-upload_post"
        aria-label="Upload Post username"
        type="text"
        className="input is-small"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Your Upload Post username (case-sensitive)"
      />
    </div>
  );
}

// ── ConnectForm ───────────────────────────────────────────────────────────────

function useConnectCredential(provider: string, repoId: string, onConnected: () => void) {
  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [uploadPostUsername, setUploadPostUsername] = useState('');
  const { loading, error, run } = useAsyncCommand();
  const [connectedNames, setConnectedNames] = useState<Record<string, string> | null>(null);
  const [syncWarning, setSyncWarning] = useState<string | null>(null);
  const canConnect = apiKey && (provider !== 'upload_post' || uploadPostUsername);

  async function handleConnect() {
    setSyncWarning(null);
    const payload = { provider, apiKey, repoId, ...(provider === 'upload_post' ? { username: uploadPostUsername } : {}) };
    const response = await run(() => invoke<SaveCredentialResponse>('save_scheduler_credential', payload));
    if (response !== null) {
      setConnectedNames(response?.account_names ?? {});
      if (response?.sync_warning) setSyncWarning(response.sync_warning);
      setTimeout(() => onConnected(), 5000);
    }
  }
  return { apiKey, setApiKey, showKey, setShowKey, uploadPostUsername, setUploadPostUsername, loading, error, connectedNames, syncWarning, canConnect, handleConnect };
}

function ConnectForm({ provider, repoId, onConnected, onCancel }: {
  provider: string; repoId: string; onConnected: () => void; onCancel: () => void;
}) {
  const { apiKey, setApiKey, showKey, setShowKey, uploadPostUsername, setUploadPostUsername,
    loading, error, connectedNames, syncWarning, canConnect, handleConnect } =
    useConnectCredential(provider, repoId, onConnected);

  return (
    <div className="mt-2 pb-2" style={{ borderBottom: '1px solid var(--bulma-border-weak)' }}>
      <div className="is-flex" style={{ gap: '0.5rem', alignItems: 'flex-end' }}>
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
          {provider === 'upload_post' && (
            <UploadPostUsernameField value={uploadPostUsername} onChange={setUploadPostUsername} />
          )}
          <label className="is-sr-only" htmlFor={`scheduler-api-key-${provider}`}>API key</label>
          <input id={`scheduler-api-key-${provider}`} aria-label="API key"
            type={showKey ? 'text' : 'password'} className="input is-small"
            value={apiKey} onChange={(e) => setApiKey(e.target.value)}
            placeholder={`Enter your ${providerLabel(provider)} API key`} />
        </div>
        <button className="button is-small is-ghost" onClick={() => setShowKey((v) => !v)}>{showKey ? 'Hide' : 'Show'}</button>
        <button className="button is-small is-primary" onClick={handleConnect} disabled={loading || !canConnect}>Connect</button>
        <button className="button is-small is-ghost" onClick={onCancel} disabled={loading}>Cancel</button>
      </div>
      {connectedNames !== null && !syncWarning && (
        <p role="status" className="is-size-7 has-text-success mt-1">{formatSuccessMessage(provider, connectedNames)}</p>
      )}
      {syncWarning && <p role="alert" className="is-size-7 has-text-warning mt-1">{syncWarning}</p>}
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

type SyncStatus = { ok: boolean; message: string };

function useSyncStatus(): { syncStatus: SyncStatus | null; showSyncStatus: (s: SyncStatus | null) => void } {
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const timerRef: MutableRefObject<ReturnType<typeof setTimeout> | null> = useRef(null);
  const showSyncStatus = useCallback((s: SyncStatus | null) => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setSyncStatus(s);
    if (s) { timerRef.current = setTimeout(() => setSyncStatus(null), 5000); }
  }, []);
  return { syncStatus, showSyncStatus };
}

// ── Main component ────────────────────────────────────────────────────────────

export default function SchedulerBlock({ projectId, isOwner }: Props) {
  const [connected, setConnected] = useState<string[]>([]);
  const [accountNames, setAccountNames] = useState<Record<string, string>>({});
  const [expandedProvider, setExpandedProvider] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const { syncStatus, showSyncStatus } = useSyncStatus();

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
          <button className="button is-small is-ghost" onClick={() => syncAccounts(projectId, setLoading, showSyncStatus, loadProfiles)} disabled={loading} aria-label="Sync accounts">
            Sync accounts
          </button>
        )}
      </div>
      {syncStatus && (
        <div
          role="alert"
          className={`notification is-light is-size-7 py-2 px-3 mb-2 ${syncStatus.ok ? 'is-success' : 'is-danger'}`}
        >
          <button className="delete is-small" onClick={() => showSyncStatus(null)} aria-label="Dismiss" />
          {syncStatus.message}
        </div>
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
