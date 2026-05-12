// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '../ipc/invoke';
import MastodonOAuthPanel from './MastodonOAuthPanel';
import SubstackNotesPanel from './SubstackNotesPanel';
import WebhookPanel from './WebhookPanel';

export const PROVIDERS = ['zernio', 'upload_post', 'publer', 'outstand'] as const;
export type Provider = (typeof PROVIDERS)[number];

const PROVIDER_NOTES: Partial<Record<Provider, string>> = {
  upload_post: '10 uploads/month free. Supports Instagram, TikTok, and YouTube.',
  publer: 'API access requires a paid plan.',
  outstand: '$5/month for 1,000 posts, then $0.01 per additional post.',
};

const PROVIDER_PLATFORMS: Record<Provider, string[]> = {
  zernio:      ['X', 'LinkedIn', 'Bluesky', 'Mastodon', 'Instagram', 'Facebook', 'Pinterest'],
  upload_post: ['X', 'Bluesky', 'LinkedIn', 'Instagram', 'TikTok', 'YouTube', 'Facebook', 'Reddit', 'Threads'],
  publer:      ['X', 'LinkedIn', 'Facebook', 'Instagram', 'Pinterest', 'TikTok', 'YouTube'],
  outstand:    ['X', 'LinkedIn', 'Instagram', 'Facebook'],
};

export interface UsageResponse {
  provider: string;
  count: number;
  limit: number | null;
  month: number;
  year: number;
}

export function UsageBadge({ usage }: { usage: UsageResponse | undefined }) {
  if (!usage || usage.limit === null) return null;
  const { count, limit } = usage;
  const atLimit = count >= limit;
  const nearLimit = count >= Math.floor(limit * 0.8);
  const countStr = count.toLocaleString();
  const limitStr = limit.toLocaleString();
  if (atLimit) {
    return <span className="is-size-7 has-text-danger">{countStr}/{limitStr} posts — Limit reached. Posts will fall back to your next configured provider.</span>;
  }
  if (nearLimit) {
    return <span className="is-size-7 has-text-warning-dark">{countStr}/{limitStr} posts used this month — approaching limit</span>;
  }
  return <span className="is-size-7 has-text-grey">{countStr}/{limitStr} posts used this month</span>;
}

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
  usage?: UsageResponse;
  note?: string;
  platforms: string[];
  onTest: () => void;
  onStartAdd: () => void;
  onSave: () => void;
  onCancelAdd: () => void;
  onKeyChange: (_key: string) => void;
  onRemove: () => void;
}

function SchedulerProviderCard({ provider, cred, usage, note, platforms, onTest, onStartAdd, onSave, onCancelAdd, onKeyChange, onRemove }: ProviderCardProps) {
  return (
    <div data-provider={provider} className="box p-4">
      <div className="is-flex is-align-items-center is-justify-content-space-between" style={{ gap: '1rem' }}>
        <div>
          <span className="has-text-weight-medium is-capitalized">{provider}</span>
          {note && <p className="is-size-7 has-text-grey mt-1">{note}</p>}
          <UsageBadge usage={usage} />
          <div className="tags mt-2" style={{ flexWrap: 'wrap', gap: '0.25rem' }}>
            {platforms.map((p) => <span key={p} className="tag is-light is-small">{p}</span>)}
          </div>
          {cred.preview
            ? <span className="is-size-7 has-text-grey">{cred.preview}</span>
            : <span className="is-size-7 has-text-grey-light">not configured</span>}
        </div>
        <div className="is-flex is-align-items-center" style={{ gap: '0.5rem', flexShrink: 0 }}>
          {cred.testResult === 'ok' && <span className="is-size-7 has-text-success">✓</span>}
          {cred.testResult === 'error' && <span className="is-size-7 has-text-danger">{cred.testError}</span>}
          {cred.preview ? (
            <>
              <button className="button is-outlined is-small" onClick={onTest} disabled={cred.testing}>Test</button>
              <button className="button is-outlined is-small" onClick={onStartAdd}>Change</button>
              <button className="button is-outlined is-small" onClick={onRemove}>Remove</button>
            </>
          ) : (
            <button className="button is-outlined is-small" onClick={onStartAdd}>+ Add</button>
          )}
        </div>
      </div>
      {cred.adding && (
        <div className="mt-3 is-flex" style={{ gap: '0.5rem' }}>
          <input type="password" value={cred.keyInput} onChange={(e) => onKeyChange(e.target.value)}
            placeholder="API key" className="input is-small" style={{ flex: 1 }} />
          <button className="button is-small is-primary" onClick={onSave}>Save</button>
          <button className="button is-ghost is-small" onClick={onCancelAdd}>Cancel</button>
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
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!provider) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', onKey);
    ref.current?.focus();
    return () => document.removeEventListener('keydown', onKey);
  }, [provider, onClose]);

  if (!provider) return null;
  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onClose} />
      <div className="modal-card" role="dialog" aria-modal="true" ref={ref} tabIndex={-1}>
        <header className="modal-card-head">
          <p className="modal-card-title">Remove {provider} API key</p>
          <button className="delete" onClick={onClose} aria-label="Close" />
        </header>
        <section className="modal-card-body">
          <p className="is-size-7 mb-3">
            This will permanently delete the API key from your macOS Keychain.
            Any repos using {provider} will stop working until a new key is added.
          </p>
          <p className="is-size-7 mb-2">Type <strong>{provider}</strong> to confirm:</p>
          <input type="text" value={input} onChange={(e) => onInputChange(e.target.value)}
            placeholder={provider} className="input is-small" autoFocus />
        </section>
        <footer className="modal-card-foot is-justify-content-flex-end" style={{ gap: '0.5rem' }}>
          <button className="button is-ghost" onClick={onClose}>Cancel</button>
          <button className="button is-danger" disabled={input !== provider} onClick={onConfirm}>Remove</button>
        </footer>
      </div>
    </div>
  );
}

const COUNTED_PROVIDERS: Provider[] = ['upload_post', 'publer', 'outstand'];

export async function loadSchedulerCreds(
  isCancelled: () => boolean,
  onCred: (provider: Provider, preview: string) => void,
): Promise<void> {
  await Promise.all(
    PROVIDERS.map(async (provider) => {
      try {
        const preview = await invoke<string>('get_scheduler_credential', { provider })
        if (!isCancelled()) onCred(provider, preview)
      } catch {
        // not configured — skip
      }
    })
  )
}

function useSchedulerCreds() {
  const init: CredentialState = { preview: null, testing: false, testResult: null, testError: null, adding: false, keyInput: '' };
  const [creds, setCreds] = useState<Record<Provider, CredentialState>>({ zernio: { ...init }, upload_post: { ...init }, publer: { ...init }, outstand: { ...init } });
  const [removeProvider, setRemoveProvider] = useState<Provider | null>(null);
  const [removeInput, setRemoveInput] = useState('');
  const [usage, setUsage] = useState<Partial<Record<Provider, UsageResponse>>>({});

  useEffect(() => {
    let cancelled = false
    const isCancelled = () => cancelled

    loadSchedulerCreds(isCancelled, (provider, preview) =>
      setCreds((prev) => ({ ...prev, [provider]: { ...prev[provider], preview } }))
    )

    void Promise.all(COUNTED_PROVIDERS.map(async (provider) => {
      try {
        const u = await invoke<UsageResponse>('get_scheduler_usage', { provider });
        if (!cancelled) setUsage((prev) => ({ ...prev, [provider]: u }));
      } catch { /* ignore — usage display is non-critical */ }
    }));

    return () => { cancelled = true }
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

  return { creds, usage, removeProvider, setRemoveProvider, removeInput, setRemoveInput, update, handleSave, handleRemove, handleTest };
}

export default function SchedulerTab() {
  const { creds, usage, removeProvider, setRemoveProvider, removeInput, setRemoveInput, update, handleSave, handleRemove, handleTest } = useSchedulerCreds();

  return (
    <>
      <div aria-hidden={removeProvider !== null ? 'true' : undefined}
        style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
        <h2 className="has-text-weight-semibold is-size-7">Default scheduler</h2>
        <p className="is-size-7 has-text-grey">
          These are your default credentials. Individual repos can use different accounts — configure per-repo in Settings → Repos → Configure.
        </p>
        <div className="notification is-info is-light is-size-7">
          <strong>macOS Keychain:</strong> API keys are stored securely in Keychain. You will be prompted once per key — click <strong>Always Allow</strong>.
        </div>
        {PROVIDERS.map((provider) => (
          <SchedulerProviderCard key={provider} provider={provider} cred={creds[provider]}
            usage={usage[provider]}
            note={PROVIDER_NOTES[provider]}
            platforms={PROVIDER_PLATFORMS[provider]}
            onTest={() => handleTest(provider)} onStartAdd={() => update(provider, { adding: true })}
            onSave={() => handleSave(provider)} onCancelAdd={() => update(provider, { adding: false, keyInput: '' })}
            onKeyChange={(key) => update(provider, { keyInput: key })}
            onRemove={() => { setRemoveInput(''); setRemoveProvider(provider); }}
          />
        ))}
        <SubstackNotesPanel />
        <WebhookPanel />
        <div className="mt-4">
          <h2 className="has-text-weight-semibold is-size-7 mb-3">Mastodon (direct API)</h2>
          <MastodonOAuthPanel />
        </div>
      </div>
      <RemoveKeyDialog
        provider={removeProvider}
        input={removeInput}
        onInputChange={setRemoveInput}
        onClose={() => { setRemoveProvider(null); setRemoveInput(''); }}
        onConfirm={() => removeProvider && handleRemove(removeProvider)}
      />
    </>
  );
}
