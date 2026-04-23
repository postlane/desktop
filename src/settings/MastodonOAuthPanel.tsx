// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Button } from '../components/catalyst/button';

type OAuthStep = 'idle' | 'code-entry' | 'connected';

interface IdleFormProps {
  instance: string;
  error: string | null;
  connecting: boolean;
  onInstanceChange: (_v: string) => void;
  onConnect: () => void;
}

function IdleForm({ instance, error, connecting, onInstanceChange, onConnect }: IdleFormProps) {
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
      </div>
      <Button onClick={onConnect} disabled={connecting}>
        {connecting ? 'Connecting…' : 'Connect'}
      </Button>
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

export default function MastodonOAuthPanel() {
  const [step, setStep] = useState<OAuthStep>('idle');
  const [instance, setInstance] = useState('');
  const [code, setCode] = useState('');
  const [acct, setAcct] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);

  async function handleConnect() {
    if (instance.includes('://')) {
      setError('Instance must be a hostname only (e.g. mastodon.social), not a URL.');
      return;
    }
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

  return (
    <div className="rounded-lg border border-zinc-200 p-4 dark:border-zinc-700">
      <h3 className="mb-3 text-sm font-medium text-zinc-900 dark:text-zinc-100">Mastodon</h3>
      {step === 'idle' && (
        <IdleForm instance={instance} error={error} connecting={connecting}
          onInstanceChange={setInstance} onConnect={handleConnect} />
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
