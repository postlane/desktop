// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';

type Step = 'loading' | 'idle' | 'instance-form' | 'code-form' | 'connected';

// ── Hook ──────────────────────────────────────────────────────────────────────

function useConnectionCheck(
  setConnectedInstance: (_v: string) => void,
  setStep: (_s: Step) => void,
) {
  useEffect(() => {
    invoke<string | null>('get_mastodon_connected_instance')
      .then((i) => { if (i) { setConnectedInstance(i); setStep('connected'); } else setStep('idle'); })
      .catch(() => setStep('idle'));
  }, [setConnectedInstance, setStep]);
}

function useMastodonRow() {
  const [step, setStep] = useState<Step>('loading');
  const [connectedInstance, setConnectedInstance] = useState('');
  const [instanceInput, setInstanceInput] = useState('');
  const [code, setCode] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useConnectionCheck(setConnectedInstance, setStep);
  function handleInstanceChange(value: string) { setInstanceInput(value); setError(null); }
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
      await invoke('exchange_mastodon_code', { instance: instanceInput, code });
      setConnectedInstance(instanceInput); setStep('connected'); setCode('');
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  async function handleDisconnect() {
    if (!window.confirm('Disconnect this Mastodon account?')) return;
    setBusy(true);
    try {
      await invoke('disconnect_mastodon', { instance: connectedInstance });
      setConnectedInstance(''); setInstanceInput(''); setStep('idle');
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  return {
    step, connectedInstance, instanceInput, code, error, busy,
    handleInstanceChange, handleConnect, handleSave,
    handleDisconnect, setCode,
    openForm: () => { setError(null); setStep('instance-form'); },
    cancel: () => { setError(null); setInstanceInput(''); setStep('idle'); },
  };
}

// ── Instance form ─────────────────────────────────────────────────────────────

function MastodonInstanceForm({ instanceInput, error, busy,
  onInstanceChange, onConnect, onCancel }: {
  instanceInput: string; error: string | null; busy: boolean;
  onInstanceChange: (_v: string) => void; onConnect: () => void; onCancel: () => void;
}) {
  return (
    <div className="pb-2" style={{ borderBottom: '1px solid var(--bulma-border-weak)' }}>
      <p className="is-size-7 has-text-grey mt-2 mb-2">
        Enter your instance hostname. You will sign in with your account in the browser.
      </p>
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        <input type="text" className="input is-small" style={{ flex: 1 }}
          placeholder="mastodon.social" value={instanceInput}
          onChange={(e) => onInstanceChange(e.target.value)} />
        <button className="button is-small is-primary" onClick={onConnect}
          disabled={busy || !instanceInput}>
          {busy ? 'Connecting…' : 'Connect to Mastodon'}
        </button>
        <button className="button is-small is-ghost" onClick={onCancel} disabled={busy}>Cancel</button>
      </div>
      {error && <p className="is-size-7 has-text-danger mt-1">{error}</p>}
    </div>
  );
}

// ── Code form ─────────────────────────────────────────────────────────────────

function MastodonCodeForm({ code, error, busy, onCodeChange, onSave }: {
  code: string; error: string | null; busy: boolean;
  onCodeChange: (_v: string) => void; onSave: () => void;
}) {
  return (
    <div className="pb-2" style={{ borderBottom: '1px solid var(--bulma-border-weak)' }}>
      <p className="is-size-7 has-text-grey mt-2 mb-2">
        A browser window opened with your Mastodon instance. Authorise Postlane, then paste the code shown here.
      </p>
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        <input type="text" className="input is-small" style={{ flex: 1 }}
          placeholder="Paste the code shown by Mastodon" value={code}
          onChange={(e) => onCodeChange(e.target.value)} />
        <button className="button is-small is-primary" onClick={onSave} disabled={busy || !code}>
          {busy ? 'Saving…' : 'Save'}
        </button>
      </div>
      {error && <p className="is-size-7 has-text-danger mt-1">{error}</p>}
    </div>
  );
}

// ── Main export ───────────────────────────────────────────────────────────────

export default function DirectChannelsBlock() {
  const row = useMastodonRow();
  const isExpanded = row.step === 'instance-form' || row.step === 'code-form';
  return (
    <div>
      <p className="is-size-6 has-text-weight-medium mb-3">Direct social channels</p>
      <div className="is-flex is-align-items-center py-2"
        style={{ gap: '0.75rem', borderBottom: isExpanded ? 'none' : '1px solid var(--bulma-border-weak)' }}>
        <span className="is-size-7" style={{ flex: 1, color: row.step === 'connected' ? 'inherit' : 'var(--bulma-grey)' }}>
          Mastodon
        </span>
        {row.step === 'connected' && (
          <>
            <span className="is-size-7 has-text-grey">{row.connectedInstance}</span>
            <button className="button is-small is-ghost has-text-danger"
              onClick={row.handleDisconnect} disabled={row.busy}>Disconnect</button>
          </>
        )}
        {row.step === 'idle' && (
          <button className="button is-small is-light" onClick={row.openForm}>Connect</button>
        )}
      </div>
      {row.step === 'instance-form' && (
        <MastodonInstanceForm
          instanceInput={row.instanceInput} error={row.error} busy={row.busy}
          onInstanceChange={row.handleInstanceChange} onConnect={row.handleConnect} onCancel={row.cancel}
        />
      )}
      {row.step === 'code-form' && (
        <MastodonCodeForm
          code={row.code} error={row.error} busy={row.busy}
          onCodeChange={row.setCode} onSave={row.handleSave}
        />
      )}
    </div>
  );
}
