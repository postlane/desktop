// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/catalyst/button';
import {
  Dialog, DialogActions, DialogBody, DialogDescription, DialogTitle,
} from '../components/catalyst/dialog';
import MastodonOAuthPanel from './MastodonOAuthPanel';
import SubstackNotesPanel from './SubstackNotesPanel';
import WebhookPanel from './WebhookPanel';

export const PROVIDERS = ['zernio', 'buffer', 'ayrshare', 'publer', 'outstand'] as const;
export type Provider = (typeof PROVIDERS)[number];

const PROVIDER_NOTES: Partial<Record<Provider, string>> = {
  publer: 'Free tier: up to 10 posts scheduled at once per account. API access may require a paid plan.',
  outstand: '$5/month for 1,000 posts, then $0.01 per additional post.',
};

export interface CredentialState {
  preview: string | null;
  testing: boolean;
  testResult: 'ok' | 'error' | null;
  testError: string | null;
  adding: boolean;
  keyInput: string;
}

interface ProviderCardProps {
  provider: Provider;
  cred: CredentialState;
  note?: string;
  onTest: () => void;
  onStartAdd: () => void;
  onSave: () => void;
  onCancelAdd: () => void;
  onKeyChange: (_key: string) => void;
  onRemove: () => void;
}

function SchedulerProviderCard({ provider, cred, note, onTest, onStartAdd, onSave, onCancelAdd, onKeyChange, onRemove }: ProviderCardProps) {
  return (
    <div className="rounded-lg border border-zinc-200 p-4 dark:border-zinc-700">
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <div>
            <span className="font-medium capitalize text-zinc-900 dark:text-zinc-100">{provider}</span>
            {note && <p className="text-xs text-zinc-400 mt-0.5">{note}</p>}
          </div>
          {cred.preview
            ? <span className="text-xs text-zinc-500">{cred.preview}</span>
            : <span className="text-xs text-zinc-400">not configured</span>}
        </div>
        <div className="flex items-center gap-2">
          {cred.testResult === 'ok' && <span className="text-xs text-green-600">✓</span>}
          {cred.testResult === 'error' && <span className="text-xs text-red-600">{cred.testError}</span>}
          {cred.preview ? (
            <>
              <Button outline onClick={onTest} disabled={cred.testing}>Test</Button>
              <Button outline onClick={onStartAdd}>Change</Button>
              <Button outline onClick={onRemove}>Remove</Button>
            </>
          ) : (
            <Button outline onClick={onStartAdd}>+ Add</Button>
          )}
        </div>
      </div>
      {cred.adding && (
        <div className="mt-3 flex gap-2">
          <input
            type="password"
            value={cred.keyInput}
            onChange={(e) => onKeyChange(e.target.value)}
            placeholder="API key"
            className="flex-1 rounded-lg border border-zinc-300 px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
          />
          <Button onClick={onSave}>Save</Button>
          <Button plain onClick={onCancelAdd}>Cancel</Button>
        </div>
      )}
    </div>
  );
}

interface RemoveKeyDialogProps {
  provider: Provider | null;
  input: string;
  onInputChange: (_v: string) => void;
  onClose: () => void;
  onConfirm: () => void;
}

function RemoveKeyDialog({ provider, input, onInputChange, onClose, onConfirm }: RemoveKeyDialogProps) {
  return (
    <Dialog open={provider !== null} onClose={onClose}>
      <DialogTitle>Remove {provider} API key</DialogTitle>
      <DialogDescription>
        This will permanently delete the API key from your macOS Keychain.
        Any repos using {provider} will stop working until a new key is added.
      </DialogDescription>
      <DialogBody>
        <p className="mb-2 text-sm text-zinc-700 dark:text-zinc-300">
          Type <strong>{provider}</strong> to confirm:
        </p>
        <input
          type="text"
          value={input}
          onChange={(e) => onInputChange(e.target.value)}
          placeholder={provider ?? ''}
          className="w-full rounded-lg border border-zinc-300 px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
          autoFocus
        />
      </DialogBody>
      <DialogActions>
        <Button plain onClick={onClose}>Cancel</Button>
        <Button color="red" disabled={input !== provider} onClick={onConfirm}>Remove</Button>
      </DialogActions>
    </Dialog>
  );
}

function useSchedulerCreds() {
  const init: CredentialState = { preview: null, testing: false, testResult: null, testError: null, adding: false, keyInput: '' };
  const [creds, setCreds] = useState<Record<Provider, CredentialState>>({ zernio: { ...init }, buffer: { ...init }, ayrshare: { ...init }, publer: { ...init }, outstand: { ...init } });
  const [removeProvider, setRemoveProvider] = useState<Provider | null>(null);
  const [removeInput, setRemoveInput] = useState('');

  useEffect(() => {
    PROVIDERS.forEach(async (provider) => {
      try { const preview = await invoke<string>('get_scheduler_credential', { provider }); setCreds((prev) => ({ ...prev, [provider]: { ...prev[provider], preview } })); }
      catch { /* not configured, skip */ }
    });
  }, []);

  function update(provider: Provider, patch: Partial<CredentialState>) {
    setCreds((prev) => ({ ...prev, [provider]: { ...prev[provider], ...patch } }));
  }

  async function handleSave(provider: Provider) {
    const key = creds[provider].keyInput;
    if (!key) return;
    try { await invoke('save_scheduler_credential', { provider, apiKey: key }); update(provider, { preview: `••••${key.slice(-4)}`, adding: false, keyInput: '' }); }
    catch (e) { console.error('save credential failed:', e); }
  }

  async function handleRemove(provider: Provider) {
    try { await invoke('delete_scheduler_credential', { provider }); update(provider, { preview: null, testResult: null }); setRemoveProvider(null); setRemoveInput(''); }
    catch (e) { update(provider, { testResult: 'error', testError: e instanceof Error ? e.message : 'Failed to remove credential' }); }
  }

  async function handleTest(provider: Provider) {
    update(provider, { testing: true, testResult: null, testError: null });
    try { await invoke('test_scheduler', { provider }); update(provider, { testing: false, testResult: 'ok' }); }
    catch (e) { update(provider, { testing: false, testResult: 'error', testError: e instanceof Error ? e.message : 'Test failed' }); }
  }

  return { creds, removeProvider, setRemoveProvider, removeInput, setRemoveInput, update, handleSave, handleRemove, handleTest };
}

export default function SchedulerTab() {
  const { creds, removeProvider, setRemoveProvider, removeInput, setRemoveInput, update, handleSave, handleRemove, handleTest } = useSchedulerCreds();

  return (
    <div className="space-y-4">
      <h2 className="text-sm font-semibold text-zinc-700 dark:text-zinc-300">Default scheduler</h2>
      <p className="text-xs text-zinc-500 dark:text-zinc-400">
        These credentials apply to all repos by default. Per-repo accounts will be configurable in v1.1.
      </p>
      <div className="rounded-lg border border-blue-200 bg-blue-50 px-3 py-2.5 text-xs text-blue-800 dark:border-blue-800 dark:bg-blue-950 dark:text-blue-200">
        <strong>macOS Keychain:</strong> API keys are stored securely in Keychain. You will be prompted once per key — click <strong>Always Allow</strong>.
      </div>
      {PROVIDERS.map((provider) => (
        <SchedulerProviderCard key={provider} provider={provider} cred={creds[provider]}
          note={PROVIDER_NOTES[provider]}
          onTest={() => handleTest(provider)} onStartAdd={() => update(provider, { adding: true })}
          onSave={() => handleSave(provider)} onCancelAdd={() => update(provider, { adding: false, keyInput: '' })}
          onKeyChange={(key) => update(provider, { keyInput: key })}
          onRemove={() => { setRemoveInput(''); setRemoveProvider(provider); }}
        />
      ))}
      <SubstackNotesPanel />
      <WebhookPanel />
      <div className="mt-6">
        <h2 className="mb-3 text-sm font-semibold text-zinc-700 dark:text-zinc-300">Mastodon (direct API)</h2>
        <MastodonOAuthPanel />
      </div>
      <RemoveKeyDialog
        provider={removeProvider}
        input={removeInput}
        onInputChange={setRemoveInput}
        onClose={() => { setRemoveProvider(null); setRemoveInput(''); }}
        onConfirm={() => removeProvider && handleRemove(removeProvider)}
      />
    </div>
  );
}
