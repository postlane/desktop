// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
import { listen } from '@tauri-apps/api/event';

interface OrgSummary {
  login: string;
  display_name: string;
  avatar_url: string;
  is_personal: boolean;
  has_project: boolean;
}

interface Props {
  projectId: string;
  onDone: (_orgLogin: string) => void;
  onClose: () => void;
  provider?: string;
}

function ScopeErrorView({ provider, onClose }: { provider: string; onClose: () => void }) {
  async function handleReauth() {
    try {
      const port = await invoke<number>('get_local_server_port');
      openUrl(`https://postlane.dev/login?desktop=1&port=${port}&provider=${provider}`).catch(console.error);
    } catch {
      openUrl(`https://postlane.dev/login?desktop=1&provider=${provider}`).catch(console.error);
    }
  }
  return (
    <div>
      <p className="mb-3 is-size-7">Sign in again to grant the <code>read:org</code> scope.</p>
      <div style={{ display: 'flex', gap: '0.5rem' }}>
        <button className="button is-primary is-small" onClick={handleReauth}>Sign in again</button>
        <button className="button is-small" onClick={onClose}>Cancel</button>
      </div>
    </div>
  );
}

function LoadErrorView({ message, onRetry, onClose }: { message: string; onRetry: () => void; onClose: () => void }) {
  return (
    <div>
      <div role="alert" className="notification is-danger is-light py-2 px-3 is-size-7 mb-3">{message}</div>
      <div style={{ display: 'flex', gap: '0.5rem' }}>
        <button className="button is-small is-light" onClick={onRetry}>Retry</button>
        <button className="button is-small" onClick={onClose}>Cancel</button>
      </div>
    </div>
  );
}

function useOrgLoader(provider: string) {
  const [orgs, setOrgs] = useState<OrgSummary[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [scopeError, setScopeError] = useState(false);
  const [retryCount, setRetryCount] = useState(0);

  useEffect(() => {
    setOrgs(null); setLoadError(null); setScopeError(false);
    invoke<OrgSummary[]>('list_provider_orgs', { provider })
      .then(setOrgs)
      .catch((err: unknown) => {
        const msg = err instanceof Error ? err.message : String(err);
        if (msg.includes('scope_not_granted')) { setScopeError(true); } else { setLoadError(msg); }
      });
  }, [provider, retryCount]);

  useEffect(() => {
    if (!scopeError) return;
    let unlisten: (() => void) | undefined;
    let mounted = true;
    listen('license:activated', () => { if (mounted) setRetryCount((c) => c + 1); })
      .then((fn) => { if (mounted) { unlisten = fn; } else { fn(); } })
      .catch(console.error);
    return () => { mounted = false; unlisten?.(); };
  }, [scopeError]);

  return { orgs, loadError, scopeError, retry: () => setRetryCount((c) => c + 1) };
}

function OrgPickerList({ orgs, selectedLogin, onSelect }: {
  orgs: OrgSummary[];
  selectedLogin: string | null;
  onSelect: (_login: string) => void;
}) {
  return (
    <div role="listbox" aria-label="Organisations" className="mb-3">
      {orgs.map((org) => (
        <button key={org.login} type="button" role="option"
          aria-selected={selectedLogin === org.login}
          onClick={() => onSelect(org.login)}
          className={`button is-fullwidth is-justify-content-flex-start mb-2 ${selectedLogin === org.login ? 'is-primary' : 'is-light'}`}
          style={{ height: 'auto', padding: '8px 12px' }}
        >
          {org.avatar_url && (
            <img src={org.avatar_url} alt={org.display_name} width={28} height={28}
              style={{ borderRadius: '50%', marginRight: 8 }} />
          )}
          <span style={{ flex: 1, textAlign: 'left' }}>
            <strong>{org.login}</strong>
            {org.is_personal && <span className="tag is-small ml-2">Personal</span>}
          </span>
        </button>
      ))}
    </div>
  );
}

export default function OrgLinkModal({ projectId, onDone, onClose, provider = 'github' }: Props) {
  const { orgs, loadError, scopeError, retry } = useOrgLoader(provider);
  const [selectedLogin, setSelectedLogin] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [connectError, setConnectError] = useState<string | null>(null);

  async function handleConnect() {
    if (!selectedLogin) return;
    setConnectError(null);
    setConnecting(true);
    try {
      await invoke('update_project_org_login', { projectId, orgLogin: selectedLogin });
      onDone(selectedLogin);
    } catch (err: unknown) {
      setConnectError(err instanceof Error ? err.message : String(err));
    } finally {
      setConnecting(false);
    }
  }

  if (scopeError) return <ScopeErrorView provider={provider} onClose={onClose} />;
  if (loadError) return <LoadErrorView message={loadError} onRetry={retry} onClose={onClose} />;
  if (orgs === null) return <p className="has-text-grey is-size-7">Loading organisations…</p>;

  return (
    <div>
      <OrgPickerList orgs={orgs} selectedLogin={selectedLogin}
        onSelect={(login) => { setSelectedLogin(login); setConnectError(null); }} />
      {connectError && (
        <div role="alert" className="notification is-danger is-light py-2 px-3 is-size-7 mb-3">
          {connectError}
        </div>
      )}
      <div style={{ display: 'flex', gap: '0.5rem' }}>
        <button className="button is-primary is-small" disabled={!selectedLogin || connecting} onClick={handleConnect}>
          {connecting ? 'Connecting…' : 'Connect org'}
        </button>
        <button className="button is-small" onClick={onClose}>Cancel</button>
      </div>
    </div>
  );
}
