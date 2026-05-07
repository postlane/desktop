// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';

type OAuthStep = 'idle' | 'code-entry' | 'connected';
type ValidationState = 'unvalidated' | 'valid' | 'invalid';

function useInstanceValidation(setError: (e: string | null) => void) {
  const [instance, setInstance] = useState('');
  const [validating, setValidating] = useState(false);
  const [validationState, setValidationState] = useState<ValidationState>('unvalidated');

  function handleInstanceChange(value: string) {
    setInstance(value);
    setValidationState('unvalidated');
    setError(null);
  }

  async function handleTestInstance() {
    if (instance.includes('://')) {
      setError('Instance must be a hostname only (e.g. mastodon.social), not a URL.');
      return;
    }
    setValidating(true);
    setValidationState('unvalidated');
    setError(null);
    try {
      await invoke('get_mastodon_char_limit', { instance });
      setValidationState('valid');
    } catch {
      setValidationState('invalid');
    } finally {
      setValidating(false);
    }
  }

  return { instance, setInstance, validating, validationState, handleInstanceChange, handleTestInstance };
}

interface IdleFormProps {
  instance: string;
  error: string | null;
  connecting: boolean;
  validating: boolean;
  validationState: ValidationState;
  onInstanceChange: (_v: string) => void;
  onConnect: () => void;
  onTestInstance: () => void;
}

function IdleForm({ instance, error, connecting, validating, validationState, onInstanceChange, onConnect, onTestInstance }: IdleFormProps) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <div>
        <input type="text" value={instance} onChange={(e) => onInstanceChange(e.target.value)}
          placeholder="mastodon.social" className="input is-small" />
        {error && <p className="is-size-7 has-text-danger mt-1">{error}</p>}
        {validationState === 'valid' && <p className="is-size-7 has-text-success mt-1">✓ Valid</p>}
        {validationState === 'invalid' && <p className="is-size-7 has-text-danger mt-1">✗ Instance not found</p>}
      </div>
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        <button className="button is-ghost is-small" onClick={onTestInstance} disabled={validating || !instance}>
          {validating ? 'Testing…' : 'Test instance'}
        </button>
        <button className="button is-primary is-small" onClick={onConnect} disabled={connecting || validationState !== 'valid'}>
          {connecting ? 'Connecting…' : 'Connect'}
        </button>
      </div>
    </div>
  );
}

interface CodeEntryProps {
  code: string;
  error: string | null;
  saving: boolean;
  onCodeChange: (_v: string) => void;
  onSave: () => void;
}

function CodeEntryForm({ code, error, saving, onCodeChange, onSave }: CodeEntryProps) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <p className="is-size-7 has-text-grey">
        A browser window opened with your Mastodon instance. Authorise Postlane, then paste the code shown here.
      </p>
      <div>
        <input type="text" value={code} onChange={(e) => onCodeChange(e.target.value)}
          placeholder="Paste the code shown by Mastodon" className="input is-small" />
        {error && <p className="is-size-7 has-text-danger mt-1">{error}</p>}
      </div>
      <button className="button is-primary is-small" onClick={onSave} disabled={saving}>
        {saving ? 'Saving…' : 'Save'}
      </button>
    </div>
  );
}

interface ConnectedViewProps {
  acct: string;
  disconnecting: boolean;
  onDisconnect: () => void;
}

function ConnectedView({ acct, disconnecting, onDisconnect }: ConnectedViewProps) {
  return (
    <div className="is-flex is-align-items-center is-justify-content-space-between">
      <span className="is-size-7 has-text-weight-medium">@{acct}</span>
      <button className="button is-outlined is-small" onClick={onDisconnect} disabled={disconnecting}>
        {disconnecting ? 'Disconnecting…' : 'Disconnect'}
      </button>
    </div>
  );
}

function useMastodonOAuth() {
  const [step, setStep] = useState<OAuthStep>('idle');
  const [code, setCode] = useState('');
  const [acct, setAcct] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);
  const { instance, setInstance, validating, validationState, handleInstanceChange, handleTestInstance } =
    useInstanceValidation(setError);

  async function handleConnect() {
    setError(null);
    setConnecting(true);
    try {
      const authUrl = await invoke<string>('register_mastodon_app', { instance });
      await openUrl(authUrl);
      setStep('code-entry');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setConnecting(false);
    }
  }

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      const fetchedAcct = await invoke<string>('exchange_mastodon_code', { instance, code });
      setAcct(fetchedAcct);
      setStep('connected');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  async function handleDisconnect() {
    if (!window.confirm('Disconnect this Mastodon account? You will need to reconnect to post again.')) {
      return;
    }
    setDisconnecting(true);
    try {
      await invoke('disconnect_mastodon', { instance });
      setStep('idle');
      setInstance('');
      setCode('');
      setAcct('');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setDisconnecting(false);
    }
  }

  return {
    step, instance, code, acct, error, connecting, saving, disconnecting,
    validating, validationState,
    handleInstanceChange, setCode,
    handleConnect, handleSave, handleDisconnect, handleTestInstance,
  };
}

export default function MastodonOAuthPanel() {
  const {
    step, instance, code, acct, error, connecting, saving, disconnecting,
    validating, validationState,
    handleInstanceChange, setCode,
    handleConnect, handleSave, handleDisconnect, handleTestInstance,
  } = useMastodonOAuth();
  return (
    <div className="box p-4">
      <h3 className="has-text-weight-medium is-size-7 mb-3">Mastodon</h3>
      {step === 'idle' && (
        <IdleForm instance={instance} error={error} connecting={connecting}
          validating={validating} validationState={validationState}
          onInstanceChange={handleInstanceChange}
          onConnect={handleConnect} onTestInstance={handleTestInstance}
        />
      )}
      {step === 'code-entry' && (
        <CodeEntryForm code={code} error={error} saving={saving}
          onCodeChange={setCode} onSave={handleSave} />
      )}
      {step === 'connected' && (
        <ConnectedView acct={acct} disconnecting={disconnecting} onDisconnect={handleDisconnect} />
      )}
    </div>
  );
}
