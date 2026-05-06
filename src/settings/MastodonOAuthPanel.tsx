// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Button } from '../components/catalyst/button';

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

function IdleForm({
  instance,
  error,
  connecting,
  validating,
  validationState,
  onInstanceChange,
  onConnect,
  onTestInstance,
}: IdleFormProps) {
  return (
    <div className="space-y-3">
      <div>
        <input
          type="text"
          value={instance}
          onChange={(e) => onInstanceChange(e.target.value)}
          placeholder="mastodon.social"
          className="w-full rounded-lg border border-zinc-300 px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
        />
        {error && <p className="mt-1 text-xs text-red-600 dark:text-red-400">{error}</p>}
        {validationState === 'valid' && (
          <p className="mt-1 text-xs text-green-600 dark:text-green-400">✓ Valid</p>
        )}
        {validationState === 'invalid' && (
          <p className="mt-1 text-xs text-red-600 dark:text-red-400">✗ Instance not found</p>
        )}
      </div>
      <div className="flex gap-2">
        <Button plain onClick={onTestInstance} disabled={validating || !instance}>
          {validating ? 'Testing…' : 'Test instance'}
        </Button>
        <Button onClick={onConnect} disabled={connecting || validationState !== 'valid'}>
          {connecting ? 'Connecting…' : 'Connect'}
        </Button>
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
    <div className="space-y-3">
      <p className="text-xs text-zinc-500 dark:text-zinc-400">
        A browser window opened with your Mastodon instance. Authorise Postlane, then paste the code shown here.
      </p>
      <div>
        <input
          type="text"
          value={code}
          onChange={(e) => onCodeChange(e.target.value)}
          placeholder="Paste the code shown by Mastodon"
          className="w-full rounded-lg border border-zinc-300 px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
        />
        {error && <p className="mt-1 text-xs text-red-600 dark:text-red-400">{error}</p>}
      </div>
      <Button onClick={onSave} disabled={saving}>
        {saving ? 'Saving…' : 'Save'}
      </Button>
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
    <div className="flex items-center justify-between">
      <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">@{acct}</span>
      <Button outline onClick={onDisconnect} disabled={disconnecting}>
        {disconnecting ? 'Disconnecting…' : 'Disconnect'}
      </Button>
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
    <div className="rounded-lg border border-zinc-200 p-4 dark:border-zinc-700">
      <h3 className="mb-3 text-sm font-medium text-zinc-900 dark:text-zinc-100">Mastodon</h3>
      {step === 'idle' && (
        <IdleForm
          instance={instance} error={error} connecting={connecting}
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
