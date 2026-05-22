// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '../ipc/invoke';

function KeyEntry({ onSaved, saving, error }: { onSaved: (_key: string) => void; saving: boolean; error: string | null }) {
  const [keyInput, setKeyInput] = useState('');
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <input type="text" aria-label="Unsplash access key" placeholder="Paste your Unsplash access key"
          value={keyInput} onChange={(e) => setKeyInput(e.target.value)}
          className="input is-small" style={{ flex: 1, maxWidth: '22rem' }} />
        <button className="button is-small" onClick={() => onSaved(keyInput)} disabled={!keyInput.trim() || saving} aria-label="Save">
          {saving ? 'Saving…' : 'Save'}
        </button>
      </div>
      {error && <p className="has-text-danger is-size-7">{error}</p>}
    </div>
  );
}

export default function ImageSearchBlock() {
  const [hasKey, setHasKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>('has_unsplash_key').then(setHasKey).catch(() => setHasKey(false));
  }, []);

  const handleSave = useCallback(async (key: string) => {
    if (!key.trim()) return;
    setSaving(true); setSaveError(null);
    try {
      await invoke('save_unsplash_key', { accessKey: key.trim() });
      setHasKey(true);
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : String(e));
    } finally { setSaving(false); }
  }, []);

  const handleRemove = useCallback(async () => {
    try { await invoke('delete_unsplash_key'); setHasKey(false); }
    catch (e) { setSaveError(e instanceof Error ? e.message : String(e)); }
  }, []);

  return (
    <section>
      <p className="has-text-weight-semibold mb-3">Image search</p>
      <p className="is-size-7 has-text-grey mb-3">
        Connect an Unsplash API key to search for photos directly from the post editor.
        Keys are stored securely in your OS keychain.
      </p>
      {hasKey && (
        <div className="is-flex is-align-items-center mb-3" style={{ gap: '0.5rem' }}>
          <span className="tag is-success is-light">Key configured</span>
          <button className="button is-ghost is-small has-text-danger" onClick={handleRemove}>Remove</button>
        </div>
      )}
      <KeyEntry onSaved={handleSave} saving={saving} error={saveError} />
    </section>
  );
}
