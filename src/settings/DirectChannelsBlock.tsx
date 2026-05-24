// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';

type Step = 'loading' | 'idle' | 'instance-form' | 'code-form' | 'connected';
type MastodonAccount = { instance: string; username: string };

// ── Hook ──────────────────────────────────────────────────────────────────────

function useConnectionCheck(
  projectId: string,
  setConnectedAccount: (_a: MastodonAccount | null) => void,
  setStep: (_s: Step) => void,
) {
  useEffect(() => {
    invoke<MastodonAccount | null>('get_mastodon_connected_account', { projectId })
      .then((a) => { if (a) { setConnectedAccount(a); setStep('connected'); } else setStep('idle'); })
      .catch(() => setStep('idle'));
  }, [projectId, setConnectedAccount, setStep]);
}

function useMastodonRow(projectId: string) {
  const [step, setStep] = useState<Step>('loading');
  const [connectedAccount, setConnectedAccount] = useState<MastodonAccount | null>(null);
  const [instanceInput, setInstanceInput] = useState('');
  const [code, setCode] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useConnectionCheck(projectId, setConnectedAccount, setStep);
  function handleInstanceChange(value: string) { setInstanceInput(value); setError(null); }
  function handleCodeChange(value: string) { setCode(value); }
  async function handleConnect() {
    if (instanceInput.includes('://')) { setError('Enter a hostname only, e.g. mastodon.social'); return; }
    setBusy(true); setError(null);
    try {
      await invoke('get_mastodon_char_limit', { instance: instanceInput });
    } catch { setError('Instance not found'); setBusy(false); return; }
    try {
      const url = await invoke<string>('register_mastodon_app', { instance: instanceInput });
      await openUrl(url);
      setStep('code-form');
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  async function handleSave() {
    setBusy(true); setError(null);
    try {
      const username = await invoke<string>('exchange_mastodon_code', { instance: instanceInput, code, projectId });
      setConnectedAccount({ instance: instanceInput, username });
      setStep('connected'); setCode('');
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  async function handleDisconnect() {
    if (!window.confirm('Disconnect this Mastodon account?')) return;
    setBusy(true);
    try {
      await invoke('disconnect_mastodon', { instance: connectedAccount?.instance ?? '', projectId });
      setConnectedAccount(null); setInstanceInput(''); setStep('idle');
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  return {
    step, connectedAccount, instanceInput, code, error, busy,
    handleInstanceChange, handleCodeChange, handleConnect, handleSave, handleDisconnect,
    openForm: () => { setError(null); setStep('instance-form'); },
    cancel: () => { setError(null); setInstanceInput(''); setCode(''); setStep('idle'); },
  };
}

// ── Connect form ──────────────────────────────────────────────────────────────

function MastodonConnectForm({ value, mode, error, busy, onChange, onSubmit, onCancel }: {
  value: string; mode: 'instance' | 'code'; error: string | null; busy: boolean;
  onChange: (_v: string) => void; onSubmit: () => void; onCancel: () => void;
}) {
  const placeholder = mode === 'instance'
    ? 'Input your Mastodon instance, for example mastodon.social'
    : 'Paste your one time code from Mastodon here';
  return (
    <div className="pb-2" style={{ borderBottom: '1px solid var(--bulma-border-weak)' }}>
      <div className="is-flex mt-2" style={{ gap: '0.5rem' }}>
        <input type="text" className="input is-small" style={{ flex: 1 }}
          placeholder={placeholder} value={value}
          onChange={(e) => onChange(e.target.value)} />
        <button className="button is-small is-primary" onClick={onSubmit}
          disabled={busy || !value}>
          {busy ? '…' : 'Submit'}
        </button>
        <button className="button is-small is-ghost" onClick={onCancel} disabled={busy}>Cancel</button>
      </div>
      {error && <p className="is-size-7 has-text-danger mt-1">{error}</p>}
    </div>
  );
}

// ── Main export ───────────────────────────────────────────────────────────────

export default function DirectChannelsBlock({ projectId }: { projectId: string }) {
  const row = useMastodonRow(projectId);
  const isExpanded = row.step === 'instance-form' || row.step === 'code-form';
  return (
    <div>
      <p className="is-size-6 has-text-weight-medium mb-3">Direct social channels</p>
      <div className="is-flex is-align-items-center py-2"
        style={{ gap: '0.75rem', borderBottom: isExpanded ? 'none' : '1px solid var(--bulma-border-weak)' }}>
        <span className="is-size-7" style={{ flex: 1, color: row.step === 'connected' ? 'inherit' : 'var(--bulma-grey)' }}>
          Mastodon
        </span>
        {row.step === 'connected' && row.connectedAccount && (
          <>
            <span className="tag is-light is-small">@{row.connectedAccount.username}</span>
            <span className="is-size-7 has-text-grey">{row.connectedAccount.instance}</span>
            <button className="button is-small is-ghost has-text-danger"
              onClick={row.handleDisconnect} disabled={row.busy}>Disconnect</button>
          </>
        )}
        {row.step === 'idle' && (
          <button className="button is-small is-light" onClick={row.openForm}>Connect</button>
        )}
      </div>
      {isExpanded && (
        <MastodonConnectForm
          mode={row.step === 'instance-form' ? 'instance' : 'code'}
          value={row.step === 'instance-form' ? row.instanceInput : row.code}
          error={row.error}
          busy={row.busy}
          onChange={row.step === 'instance-form' ? row.handleInstanceChange : row.handleCodeChange}
          onSubmit={row.step === 'instance-form' ? row.handleConnect : row.handleSave}
          onCancel={row.cancel}
        />
      )}
    </div>
  );
}
