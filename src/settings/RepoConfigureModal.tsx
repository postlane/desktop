// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { VoiceGuideSection } from './VoiceGuideSection';

const CONFIGURE_PROVIDERS = ['zernio', 'buffer', 'ayrshare', 'publer', 'outstand', 'substack_notes'] as const;

function friendlyKeychainError(raw: string): string {
  const lower = raw.toLowerCase();
  if (lower.includes('locked')) return 'Keychain locked — unlock it and try again.';
  if (lower.includes('permission denied') || lower.includes('access denied')) {
    return 'Access denied to keychain — check your system permissions.';
  }
  return raw;
}
type ConfigureProvider = typeof CONFIGURE_PROVIDERS[number];

interface Props {
  repoId: string;
  repoName: string;
  currentProvider: string | null;
  projectId?: string;
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
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <p className="is-size-7 has-text-grey">
        No scheduler configured. Switch to the Default scheduler tab to set one up, then come back here.
      </p>
      <button className="button is-outlined is-small" onClick={onClose}>Close and open Default scheduler</button>
    </div>
  );
}

function DefaultView({ onSwitchToCustom }: { onSwitchToCustom: () => void }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <p className="is-size-7 has-text-grey">Using default credentials from Settings → Default scheduler</p>
      <button className="button is-outlined is-small" onClick={onSwitchToCustom}>Use a different account</button>
    </div>
  );
}

function ConfiguredView({ maskedKey, removeError, onChange, onRemove }: {
  maskedKey: string; removeError: string | null; onChange: () => void; onRemove: () => void;
}) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
      <p className="is-size-7">
        Using separate account <span className="is-family-monospace has-text-grey">({maskedKey})</span>
      </p>
      {removeError && <p className="is-size-7 has-text-danger">{removeError}</p>}
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        <button className="button is-outlined is-small" onClick={onChange}>Change</button>
        <button className="button is-outlined is-small" onClick={onRemove}>Remove</button>
      </div>
    </div>
  );
}

function ProviderSelect({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <div className="field">
      <label htmlFor="scheduler-provider" className="label is-small">Provider</label>
      <div className="control">
        <div className="select is-small">
          <select id="scheduler-provider" aria-label="Provider" value={value} onChange={(e) => onChange(e.target.value)}>
            {CONFIGURE_PROVIDERS.map((p) => (
              <option key={p} value={p}>{PROVIDER_LABELS[p]}</option>
            ))}
          </select>
        </div>
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
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<'ok' | 'error' | null>(null);
  const [testError, setTestError] = useState<string | null>(null);

  async function handleSave() {
    if (!keyInput.trim()) return;
    setSaving(true); setSaveError(null);
    try {
      await invoke('save_repo_scheduler_key', { repoId, provider, key: keyInput.trim() });
      onSaved(provider, `••••••••${keyInput.slice(-4)}`);
    } catch (e) {
      setSaveError(friendlyKeychainError(e instanceof Error ? e.message : 'Failed to save'));
    } finally { setSaving(false); }
  }

  async function handleTest() {
    setTesting(true); setTestResult(null); setTestError(null);
    try {
      await invoke('test_scheduler', { provider, repoId });
      setTestResult('ok');
    } catch (e) {
      setTestError(friendlyKeychainError(e instanceof Error ? e.message : 'Test failed'));
      setTestResult('error');
    } finally { setTesting(false); }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <ProviderSelect value={provider} onChange={(v) => { setProvider(v); setTestResult(null); setTestError(null); }} />
      <input type="password" value={keyInput} onChange={(e) => setKeyInput(e.target.value)}
        placeholder="API key" className="input is-small" />
      {saveError && <p className="is-size-7 has-text-danger">{saveError}</p>}
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <button className="button is-primary is-small" onClick={handleSave} disabled={saving || !keyInput.trim()}>{saving ? 'Saving…' : 'Save'}</button>
        <button className="button is-outlined is-small" onClick={handleTest} disabled={testing}>Test connection</button>
        {testResult === 'ok' && <span className="is-size-7 has-text-success">Provider recognized</span>}
        {testResult === 'error' && <span className="is-size-7 has-text-danger">{testError ?? 'Failed'}</span>}
        <button className="button is-ghost is-small" onClick={onCancel}>Cancel</button>
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
  if (p.loading) return <p role="status" className="is-size-7 has-text-grey">Loading…</p>;
  if (p.showForm) return <CustomForm repoId={p.repoId} initialProvider={p.activeProvider} onSaved={p.onSaved} onCancel={p.onCancelForm} />;
  if (p.mode === 'custom' && p.maskedKey) return <ConfiguredView maskedKey={p.maskedKey} removeError={p.removeError} onChange={p.onSwitchToCustom} onRemove={p.onRemove} />;
  return <DefaultView onSwitchToCustom={p.onSwitchToCustom} />;
}

function useConfigureModal(repoId: string, currentProvider: string | null, onClose: () => void, onCredentialChange?: () => void) {
  const [mode, setMode] = useState<SchedulerMode>('default');
  const [maskedKey, setMaskedKey] = useState<string | null>(null);
  const [activeProvider, setActiveProvider] = useState(currentProvider ?? CONFIGURE_PROVIDERS[0]);
  const [showForm, setShowForm] = useState(false);
  const [loading, setLoading] = useState(!!currentProvider);
  const [removeError, setRemoveError] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', onKey);
    ref.current?.focus();
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose]);

  useEffect(() => {
    if (!currentProvider) return;
    invoke<string | null>('get_per_repo_scheduler_key', { repoId, provider: currentProvider })
      .then((key) => { if (key) { setMaskedKey(key); setMode('custom'); } })
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
      setRemoveError(friendlyKeychainError(e instanceof Error ? e.message : 'Failed to remove credential'));
    }
  }

  function handleSaved(provider: string, masked: string) {
    setActiveProvider(provider); setMaskedKey(masked); setMode('custom'); setShowForm(false);
    onCredentialChange?.();
  }

  return { mode, maskedKey, activeProvider, showForm, setShowForm, loading, removeError, ref, handleRemove, handleSaved };
}

export default function RepoConfigureModal({ repoId, repoName, currentProvider, projectId, onClose, onCredentialChange }: Props) {
  const { mode, maskedKey, activeProvider, showForm, setShowForm, loading, removeError, ref, handleRemove, handleSaved } =
    useConfigureModal(repoId, currentProvider, onClose, onCredentialChange);

  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onClose} />
      <div className="modal-card" role="dialog" aria-modal="true" ref={ref} tabIndex={-1}>
        <header className="modal-card-head">
          <p className="modal-card-title">Configure {repoName}</p>
          <button className="delete" onClick={onClose} aria-label="Close" />
        </header>
        <section className="modal-card-body">
          <h3 className="has-text-weight-medium is-size-7 mb-3">Scheduler</h3>
          <SchedulerBody
            currentProvider={currentProvider} loading={loading} mode={mode}
            maskedKey={maskedKey} showForm={showForm} removeError={removeError}
            repoId={repoId} activeProvider={activeProvider}
            onClose={onClose} onSwitchToCustom={() => setShowForm(true)}
            onRemove={handleRemove} onSaved={handleSaved} onCancelForm={() => setShowForm(false)}
          />
          {projectId && <VoiceGuideSection projectId={projectId} />}
        </section>
        <footer className="modal-card-foot is-justify-content-flex-end">
          <button className="button is-ghost" onClick={onClose}>Close</button>
        </footer>
      </div>
    </div>
  );
}
