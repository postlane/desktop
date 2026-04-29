// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/catalyst/button';
import {
  Dialog, DialogActions, DialogBody, DialogTitle,
} from '../components/catalyst/dialog';
const CONFIGURE_PROVIDERS = ['zernio', 'buffer', 'ayrshare', 'publer', 'outstand', 'substack_notes'] as const;
type ConfigureProvider = typeof CONFIGURE_PROVIDERS[number];

interface Props {
  repoId: string;
  repoName: string;
  currentProvider: string | null;
  onClose: () => void;
  onCredentialChange?: () => void;
}

type SchedulerMode = 'default' | 'custom';

const PROVIDER_LABELS: Record<ConfigureProvider, string> = {
  zernio: 'Zernio', buffer: 'Buffer', ayrshare: 'Ayrshare',
  publer: 'Publer', outstand: 'Outstand', substack_notes: 'Substack Notes',
};

function NoProviderView({ onClose }: { onClose: () => void }) {
  return (
    <div className="space-y-3">
      <p className="text-sm text-zinc-500 dark:text-zinc-400">No scheduler configured for this repo.</p>
      <Button outline onClick={onClose}>Set up default scheduler</Button>
    </div>
  );
}

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

function ConfiguredView({ maskedKey, removeError, onChange, onRemove }: {
  maskedKey: string; removeError: string | null; onChange: () => void; onRemove: () => void;
}) {
  return (
    <div className="space-y-2">
      <p className="text-sm text-zinc-700 dark:text-zinc-300">
        Using separate account <span className="font-mono text-xs text-zinc-500">({maskedKey})</span>
      </p>
      {removeError && <p className="text-xs text-red-600">{removeError}</p>}
      <div className="flex gap-2">
        <Button outline onClick={onChange}>Change</Button>
        <Button outline onClick={onRemove}>Remove</Button>
      </div>
    </div>
  );
}

function ProviderSelect({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <div>
      <label htmlFor="scheduler-provider" className="block text-xs font-medium text-zinc-600 dark:text-zinc-400 mb-1">
        Provider
      </label>
      <select id="scheduler-provider" aria-label="Provider" value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
      >
        {CONFIGURE_PROVIDERS.map((p) => (
          <option key={p} value={p}>{PROVIDER_LABELS[p]}</option>
        ))}
      </select>
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
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<'ok' | 'error' | null>(null);

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

  async function handleTest() {
    setTesting(true); setTestResult(null);
    try { await invoke('test_scheduler', { provider, repoId }); setTestResult('ok'); }
    catch { setTestResult('error'); }
    finally { setTesting(false); }
  }

  return (
    <div className="space-y-3">
      <ProviderSelect value={provider} onChange={(v) => { setProvider(v); setTestResult(null); }} />
      <input
        type="password"
        value={keyInput}
        onChange={(e) => setKeyInput(e.target.value)}
        placeholder="API key"
        className="w-full rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
      />
      {saveError && <p className="text-xs text-red-600">{saveError}</p>}
      <div className="flex items-center gap-2">
        <Button onClick={handleSave} disabled={saving || !keyInput.trim()}>{saving ? 'Saving…' : 'Save'}</Button>
        <Button outline onClick={handleTest} disabled={testing}>Test connection</Button>
        {testResult === 'ok' && <span className="text-xs text-green-600">✓</span>}
        {testResult === 'error' && <span className="text-xs text-red-600">Failed</span>}
        <Button plain onClick={onCancel}>Cancel</Button>
      </div>
    </div>
  );
}

type SchedulerBodyProps = {
  currentProvider: string | null; loading: boolean; mode: SchedulerMode;
  maskedKey: string | null; showForm: boolean; removeError: string | null;
  repoId: string; activeProvider: string;
  onClose: () => void; onSwitchToCustom: () => void;
  onRemove: () => void; onSaved: (provider: string, masked: string) => void; onCancelForm: () => void;
};

function SchedulerBody(p: SchedulerBodyProps) {
  if (!p.currentProvider) return <NoProviderView onClose={p.onClose} />;
  if (p.loading) return <p role="status" className="text-xs text-zinc-400">Loading…</p>;
  if (p.showForm) return <CustomForm repoId={p.repoId} initialProvider={p.activeProvider} onSaved={p.onSaved} onCancel={p.onCancelForm} />;
  if (p.mode === 'custom' && p.maskedKey) return <ConfiguredView maskedKey={p.maskedKey} removeError={p.removeError} onChange={p.onSwitchToCustom} onRemove={p.onRemove} />;
  return <DefaultView onSwitchToCustom={p.onSwitchToCustom} />;
}

export default function RepoConfigureModal({ repoId, repoName, currentProvider, onClose, onCredentialChange }: Props) {
  const [mode, setMode] = useState<SchedulerMode>('default');
  const [maskedKey, setMaskedKey] = useState<string | null>(null);
  const [activeProvider, setActiveProvider] = useState(currentProvider ?? CONFIGURE_PROVIDERS[0]);
  const [showForm, setShowForm] = useState(false);
  const [loading, setLoading] = useState(!!currentProvider);
  const [removeError, setRemoveError] = useState<string | null>(null);

  useEffect(() => {
    if (!currentProvider) return;
    invoke<string | null>('get_per_repo_scheduler_key', { repoId, provider: currentProvider })
      .then((key) => {
        if (key) { setMaskedKey(key); setMode('custom'); }
      })
      .catch(() => { /* non-critical */ })
      .finally(() => setLoading(false));
  }, [repoId, currentProvider]);

  async function handleRemove() {
    setRemoveError(null);
    try {
      await invoke('remove_repo_scheduler_key', { repoId, provider: activeProvider });
      setMaskedKey(null); setMode('default'); setShowForm(false);
      onCredentialChange?.();
    } catch (e) {
      setRemoveError(e instanceof Error ? e.message : 'Failed to remove credential');
    }
  }

  function handleSaved(provider: string, masked: string) {
    setActiveProvider(provider); setMaskedKey(masked); setMode('custom'); setShowForm(false);
    onCredentialChange?.();
  }

  return (
    <Dialog open onClose={onClose}>
      <DialogTitle>Configure {repoName}</DialogTitle>
      <DialogBody>
        <h3 className="mb-3 text-sm font-medium text-zinc-700 dark:text-zinc-300">Scheduler</h3>
        <SchedulerBody
          currentProvider={currentProvider} loading={loading} mode={mode}
          maskedKey={maskedKey} showForm={showForm} removeError={removeError}
          repoId={repoId} activeProvider={activeProvider}
          onClose={onClose} onSwitchToCustom={() => setShowForm(true)}
          onRemove={handleRemove} onSaved={handleSaved} onCancelForm={() => setShowForm(false)}
        />
      </DialogBody>
      <DialogActions>
        <Button plain onClick={onClose}>Close</Button>
      </DialogActions>
    </Dialog>
  );
}
