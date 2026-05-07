// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type Provider = 'zernio' | 'publer' | 'outstand' | 'webhook' | 'mastodon';

interface Profile {
  id: string;
  name: string;
}

interface ConnectionResult {
  profiles: Profile[];
}

interface Props {
  workspaceId: string;
  provider: Provider;
  onSuccess: (provider: string) => void;
  onCancel: () => void;
}

interface KeyEntryProps {
  apiKey: string;
  error: string | null;
  onKeyChange: (v: string) => void;
  onConnect: () => void;
  onCancel: () => void;
}

function KeyEntry({ apiKey, error, onKeyChange, onConnect, onCancel }: KeyEntryProps) {
  return (
    <div>
      {error && <div role="alert" className="notification is-danger is-light py-2 px-3 is-size-7 mb-3">{error}</div>}
      <div className="field">
        <label className="label is-small">API key</label>
        <div className="control">
          <input className="input is-small" type="text" value={apiKey}
            onChange={(e) => onKeyChange(e.target.value)} placeholder="Paste your API key here" />
        </div>
      </div>
      <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
        <button className="button is-primary is-small" onClick={onConnect} disabled={apiKey.trim().length === 0}>Connect</button>
        <button className="button is-light is-small" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}

interface ProfilesProps {
  profiles: Profile[];
  selectedIds: Set<string>;
  error: string | null;
  saving: boolean;
  onToggle: (id: string) => void;
  onSave: () => void;
  onCancel: () => void;
}

function ProfileList({ profiles, selectedIds, error, saving, onToggle, onSave, onCancel }: ProfilesProps) {
  return (
    <div>
      <p className="is-size-7 has-text-grey mb-3">Select profiles to enable:</p>
      {profiles.map((p) => (
        <label key={p.id} className="checkbox mb-2 is-block">
          <input type="checkbox" checked={selectedIds.has(p.id)} onChange={() => onToggle(p.id)} style={{ marginRight: 8 }} />
          {p.name}
        </label>
      ))}
      {error && <div role="alert" className="notification is-danger is-light py-2 px-3 is-size-7 mt-2">{error}</div>}
      <div className="mt-4" style={{ display: 'flex', gap: 8 }}>
        <button className="button is-primary is-small" onClick={onSave} disabled={saving || selectedIds.size === 0}>Save</button>
        <button className="button is-light is-small" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}

type Phase = 'key-entry' | 'connecting' | 'profiles';

export default function SchedulerConnect({ workspaceId, provider, onSuccess, onCancel }: Props) {
  const [apiKey, setApiKey] = useState('');
  const [phase, setPhase] = useState<Phase>('key-entry');
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function handleConnect() {
    setError(null);
    setPhase('connecting');
    try {
      const result = await invoke<ConnectionResult>('test_scheduler_connection', { provider, apiKey, workspaceId });
      setProfiles(result.profiles);
      setSelectedIds(new Set(result.profiles.map((p) => p.id)));
      setPhase('profiles');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setPhase('key-entry');
    }
  }

  async function handleSave() {
    setSaving(true);
    try {
      await invoke('save_scheduler_profiles', { workspaceId, provider, selectedProfiles: Array.from(selectedIds) });
      onSuccess(provider);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  function toggleProfile(id: string) {
    setSelectedIds((prev) => { const next = new Set(prev); if (next.has(id)) { next.delete(id); } else { next.add(id); } return next; });
  }

  if (phase === 'connecting') {
    return <div className="is-flex is-align-items-center" style={{ gap: 8 }}><span className="is-size-7 has-text-grey">Connecting...</span></div>;
  }
  if (phase === 'profiles') {
    return <ProfileList profiles={profiles} selectedIds={selectedIds} error={error} saving={saving}
      onToggle={toggleProfile} onSave={handleSave} onCancel={onCancel} />;
  }
  return <KeyEntry apiKey={apiKey} error={error} onKeyChange={setApiKey} onConnect={handleConnect} onCancel={onCancel} />;
}
