// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';

type Provider = 'zernio' | 'publer' | 'outstand' | 'upload_post' | 'webhook' | 'mastodon';

const PROVIDER_HINTS: Partial<Record<Provider, { text: string; url: string; linkLabel: string }>> = {
  zernio: {
    text: 'See our documentation on how to set up Zernio. It has the most flexibility of all schedulers.',
    url: 'https://docs.postlane.dev/scheduling/zernio',
    linkLabel: 'Zernio setup docs',
  },
  publer: {
    text: 'See our documentation on how to set up Publer.',
    url: 'https://docs.postlane.dev/scheduling/publer',
    linkLabel: 'Publer setup docs',
  },
  upload_post: {
    text: 'See our documentation on how to set up Upload Post.',
    url: 'https://docs.postlane.dev/scheduling/upload-post',
    linkLabel: 'Upload Post setup docs',
  },
};

interface KeyEntryProps {
  provider: Provider;
  apiKey: string;
  saving: boolean;
  error: string | null;
  onKeyChange: (v: string) => void;
  onConnect: () => void;
  onCancel: () => void;
}

function KeyEntry({ provider, apiKey, saving, error, onKeyChange, onConnect, onCancel }: KeyEntryProps) {
  const hint = PROVIDER_HINTS[provider];
  return (
    <div>
      {error && <div role="alert" className="notification is-danger is-light py-2 px-3 is-size-7 mb-3">{error}</div>}
      <div className="field">
        <label className="label is-small">API key</label>
        <div className="control">
          <input className="input is-small" type="text" value={apiKey}
            onChange={(e) => onKeyChange(e.target.value)} placeholder="Paste your API key here" />
        </div>
        {hint && (
          <p className="is-size-7 has-text-grey mt-2">
            {hint.text}{' '}
            <a className="has-text-link" href={hint.url}
              onClick={(e) => { e.preventDefault(); openUrl(hint.url).catch(console.error); }}>
              {hint.linkLabel}
            </a>
          </p>
        )}
      </div>
      <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
        <button className="button is-primary is-small" onClick={onConnect}
          disabled={saving || apiKey.trim().length === 0}>
          {saving ? 'Saving…' : 'Connect'}
        </button>
        <button className="button is-light is-small" onClick={onCancel} disabled={saving}>Cancel</button>
      </div>
    </div>
  );
}

interface Props {
  workspaceId: string;
  provider: Provider;
  onSuccess: (provider: string) => void;
  onCancel: () => void;
}

export default function SchedulerConnect({ workspaceId, provider, onSuccess, onCancel }: Props) {
  const [apiKey, setApiKey] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleConnect() {
    setError(null);
    setSaving(true);
    try {
      await invoke('save_scheduler_credential', { provider, apiKey, repoId: workspaceId });
      onSuccess(provider);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setSaving(false);
    }
  }

  return (
    <KeyEntry provider={provider} apiKey={apiKey} saving={saving} error={error}
      onKeyChange={setApiKey} onConnect={handleConnect} onCancel={onCancel} />
  );
}
