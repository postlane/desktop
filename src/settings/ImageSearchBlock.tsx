// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faArrowUpRightFromSquare } from '@fortawesome/free-solid-svg-icons';

const UNSPLASH_URL = 'https://unsplash.com';

function UnsplashLink() {
  return (
    <button className="button is-small is-ghost"
      style={{ padding: '0 0.25rem', color: 'var(--bulma-grey)' }}
      onClick={() => openUrl(UNSPLASH_URL)} aria-label="Open Unsplash website">
      <FontAwesomeIcon icon={faArrowUpRightFromSquare} size="xs" />
    </button>
  );
}

function KeyForm({ onConnected, onCancel }: { onConnected: () => void; onCancel: () => void }) {
  const [keyInput, setKeyInput] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleConnect() {
    if (!keyInput.trim()) return;
    setSaving(true);
    setError(null);
    try {
      await invoke('save_unsplash_key', { accessKey: keyInput.trim() });
      onConnected();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="mt-2 pb-2" style={{ borderBottom: '1px solid var(--bulma-border-weak)' }}>
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        <label className="is-sr-only" htmlFor="unsplash-access-key">Unsplash access key</label>
        <input id="unsplash-access-key" aria-label="Unsplash access key"
          type={showKey ? 'text' : 'password'}
          className="input is-small" value={keyInput} onChange={(e) => setKeyInput(e.target.value)}
          placeholder="Paste your Unsplash access key" style={{ flex: 1 }} />
        <button className="button is-small is-ghost" onClick={() => setShowKey((v) => !v)}>
          {showKey ? 'Hide' : 'Show'}
        </button>
        <button className="button is-small is-primary" onClick={handleConnect}
          disabled={saving || !keyInput.trim()}>
          Connect
        </button>
        <button className="button is-small is-ghost" onClick={onCancel} disabled={saving}>
          Cancel
        </button>
      </div>
      {error && <p role="alert" className="is-size-7 has-text-danger mt-1">{error}</p>}
    </div>
  );
}

function ConnectedRow({ expanded, onExpand, onCancel, onDisconnected }: {
  expanded: boolean;
  onExpand: () => void;
  onCancel: () => void;
  onDisconnected: () => void;
}) {
  async function handleDisconnect() {
    try { await invoke('delete_unsplash_key'); onDisconnected(); }
    catch { /* non-fatal */ }
  }
  return (
    <div>
      <div className="is-flex is-align-items-center py-2"
        style={{ gap: '0.75rem', borderBottom: expanded ? 'none' : '1px solid var(--bulma-border-weak)' }}>
        <span className="is-size-7" style={{ flex: 1 }}>Unsplash</span>
        <UnsplashLink />
        {!expanded && (
          <>
            <button className="button is-small is-ghost" onClick={onExpand}>Change key</button>
            <button className="button is-small is-ghost has-text-danger" onClick={handleDisconnect}>
              Disconnect
            </button>
          </>
        )}
      </div>
      {expanded && <KeyForm onConnected={onCancel} onCancel={onCancel} />}
    </div>
  );
}

function AvailableRow({ expanded, onExpand, onConnected, onCancel }: {
  expanded: boolean;
  onExpand: () => void;
  onConnected: () => void;
  onCancel: () => void;
}) {
  return (
    <div>
      <div className="is-flex is-align-items-center py-2"
        style={{ gap: '0.75rem', borderBottom: expanded ? 'none' : '1px solid var(--bulma-border-weak)' }}>
        <span className="is-size-7 has-text-grey" style={{ flex: 1 }}>Unsplash</span>
        <UnsplashLink />
        {!expanded && (
          <button className="button is-small is-light" onClick={onExpand}>Connect</button>
        )}
      </div>
      {expanded && <KeyForm onConnected={onConnected} onCancel={onCancel} />}
    </div>
  );
}

export default function ImageSearchBlock() {
  const [hasKey, setHasKey] = useState(false);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    invoke<boolean>('has_unsplash_key').then(setHasKey).catch(() => setHasKey(false));
  }, []);

  const handleConnected = useCallback(() => { setHasKey(true); setExpanded(false); }, []);
  const handleDisconnected = useCallback(() => { setHasKey(false); setExpanded(false); }, []);

  return (
    <section>
      <p className="is-size-6 has-text-weight-medium mb-3">Image search</p>
      {hasKey
        ? <ConnectedRow expanded={expanded} onExpand={() => setExpanded(true)}
            onCancel={() => setExpanded(false)} onDisconnected={handleDisconnected} />
        : <AvailableRow expanded={expanded} onExpand={() => setExpanded(true)}
            onConnected={handleConnected} onCancel={() => setExpanded(false)} />
      }
    </section>
  );
}
