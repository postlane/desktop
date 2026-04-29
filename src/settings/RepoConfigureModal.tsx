// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/catalyst/button';
import {
  Dialog, DialogActions, DialogBody, DialogTitle,
} from '../components/catalyst/dialog';
import { PROVIDERS } from './SchedulerTab';

interface Props {
  repoId: string;
  repoName: string;
  currentProvider: string | null;
  onClose: () => void;
}

type SchedulerMode = 'default' | 'custom';

const PROVIDER_LABELS: Record<string, string> = {
  zernio: 'Zernio', buffer: 'Buffer', ayrshare: 'Ayrshare',
  publer: 'Publer', outstand: 'Outstand', substack_notes: 'Substack Notes',
};

function DefaultView({ onSwitchToCustom }: { onSwitchToCustom: () => void }) {
  return (
    <div className="space-y-3">
      <p className="text-sm text-zinc-500 dark:text-zinc-400">
        Using default credentials from Settings → Default scheduler
      </p>
      <Button outline onClick={onSwitchToCustom}>Use a different account</Button>
    </div>
  );
}

function ConfiguredView({ maskedKey, onChange, onRemove }: {
  maskedKey: string; onChange: () => void; onRemove: () => void;
}) {
  return (
    <div className="space-y-2">
      <p className="text-sm text-zinc-700 dark:text-zinc-300">
        Using separate account <span className="font-mono text-xs text-zinc-500">({maskedKey})</span>
      </p>
      <div className="flex gap-2">
        <Button outline onClick={onChange}>Change</Button>
        <Button outline onClick={onRemove}>Remove</Button>
      </div>
    </div>
  );
}

function CustomForm({ repoId, initialProvider, onSaved, onCancel }: {
  repoId: string;
  initialProvider: string;
  onSaved: (provider: string, masked: string) => void;
  onCancel: () => void;
}) {
  const [provider, setProvider] = useState(initialProvider);
  const [keyInput, setKeyInput] = useState('');
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  async function handleSave() {
    if (!keyInput.trim()) return;
    setSaving(true); setSaveError(null);
    try {
      await invoke('save_repo_scheduler_key', { repoId, provider, key: keyInput.trim() });
      const masked = `••••••••${keyInput.slice(-4)}`;
      onSaved(provider, masked);
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : 'Failed to save');
    } finally { setSaving(false); }
  }

  return (
    <div className="space-y-3">
      <div>
        <label htmlFor="scheduler-provider" className="block text-xs font-medium text-zinc-600 dark:text-zinc-400 mb-1">
          Provider
        </label>
        <select
          id="scheduler-provider"
          aria-label="Provider"
          value={provider}
          onChange={(e) => setProvider(e.target.value)}
          className="w-full rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
        >
          {PROVIDERS.map((p) => (
            <option key={p} value={p}>{PROVIDER_LABELS[p] ?? p}</option>
          ))}
        </select>
      </div>
      <input
        type="password"
        value={keyInput}
        onChange={(e) => setKeyInput(e.target.value)}
        placeholder="API key"
        className="w-full rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
      />
      {saveError && <p className="text-xs text-red-600">{saveError}</p>}
      <div className="flex gap-2">
        <Button onClick={handleSave} disabled={saving || !keyInput.trim()}>{saving ? 'Saving…' : 'Save'}</Button>
        <Button plain onClick={onCancel}>Cancel</Button>
      </div>
    </div>
  );
}

export default function RepoConfigureModal({ repoId, repoName, currentProvider, onClose }: Props) {
  const [mode, setMode] = useState<SchedulerMode>('default');
  const [maskedKey, setMaskedKey] = useState<string | null>(null);
  const [activeProvider, setActiveProvider] = useState(currentProvider ?? PROVIDERS[0]);
  const [showForm, setShowForm] = useState(false);

  useEffect(() => {
    if (!currentProvider) return;
    invoke<string | null>('get_per_repo_scheduler_key', { repoId, provider: currentProvider })
      .then((key) => {
        if (key) { setMaskedKey(key); setMode('custom'); }
      })
      .catch(() => { /* non-critical */ });
  }, [repoId, currentProvider]);

  async function handleRemove() {
    try {
      await invoke('remove_repo_scheduler_key', { repoId, provider: activeProvider });
      setMaskedKey(null); setMode('default'); setShowForm(false);
    } catch { /* non-critical */ }
  }

  function handleSaved(provider: string, masked: string) {
    setActiveProvider(provider); setMaskedKey(masked); setMode('custom'); setShowForm(false);
  }

  return (
    <Dialog open onClose={onClose}>
      <DialogTitle>Configure {repoName}</DialogTitle>
      <DialogBody>
        <h3 className="mb-3 text-sm font-medium text-zinc-700 dark:text-zinc-300">Scheduler</h3>
        {mode === 'default' && !showForm && (
          <DefaultView onSwitchToCustom={() => setShowForm(true)} />
        )}
        {mode === 'custom' && maskedKey && !showForm && (
          <ConfiguredView maskedKey={maskedKey} onChange={() => setShowForm(true)} onRemove={handleRemove} />
        )}
        {showForm && (
          <CustomForm
            repoId={repoId}
            initialProvider={activeProvider}
            onSaved={handleSaved}
            onCancel={() => setShowForm(false)}
          />
        )}
      </DialogBody>
      <DialogActions>
        <Button plain onClick={onClose}>Close</Button>
      </DialogActions>
    </Dialog>
  );
}
