// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { UsageBadge, type UsageResponse } from './SchedulerTab';

type PanelState = 'idle' | 'adding' | 'configured';

interface IdleViewProps {
  onStartAdd: () => void;
}

function IdleView({ onStartAdd }: IdleViewProps) {
  return <button className="button is-outlined is-small" onClick={onStartAdd}>+ Add</button>;
}

interface AddingFormProps {
  url: string;
  urlError: string | null;
  saveError: string | null;
  saving: boolean;
  onUrlChange: (_v: string) => void;
  onSave: () => void;
  onCancel: () => void;
}

function AddingForm({ url, urlError, saveError, saving, onUrlChange, onSave, onCancel }: AddingFormProps) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <p className="is-size-7 has-text-grey">
        Enter a webhook URL. Postlane will POST the scheduled content as JSON to this endpoint.
      </p>
      <div>
        <input type="url" value={url} onChange={(e) => onUrlChange(e.target.value)}
          placeholder="https://hooks.example.com/webhook" className="input is-small" />
        {urlError && <p className="is-size-7 has-text-danger mt-1">{urlError}</p>}
        {saveError && <p className="is-size-7 has-text-danger mt-1">{saveError}</p>}
      </div>
      <div className="is-flex" style={{ gap: '0.5rem' }}>
        <button className="button is-primary is-small" onClick={onSave} disabled={saving}>{saving ? 'Saving…' : 'Save'}</button>
        <button className="button is-ghost is-small" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}

interface ConfiguredViewProps {
  preview: string;
  testing: boolean;
  testResult: 'ok' | 'error' | null;
  testError: string | null;
  onTest: () => void;
  onChange: () => void;
  onRemove: () => void;
}

function ConfiguredView({ preview, testing, testResult, testError, onTest, onChange, onRemove }: ConfiguredViewProps) {
  return (
    <div className="is-flex is-align-items-center is-justify-content-space-between" style={{ gap: '1rem' }}>
      <span className="is-size-7 has-text-grey" style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>{preview}</span>
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem', flexShrink: 0 }}>
        {testResult === 'ok' && <span className="is-size-7 has-text-success">✓</span>}
        {testResult === 'error' && <span className="is-size-7 has-text-danger">{testError}</span>}
        <button className="button is-outlined is-small" onClick={onTest} disabled={testing}>Test</button>
        <button className="button is-outlined is-small" onClick={onChange}>Change</button>
        <button className="button is-outlined is-small" onClick={onRemove}>Remove</button>
      </div>
    </div>
  );
}

function validateUrl(url: string): string | null {
  if (!url) return null;
  if (!url.startsWith('https://')) return 'Webhook URL must use https://';
  return null;
}

function maskWebhookUrl(url: string): string {
  try {
    const { protocol, hostname } = new URL(url);
    const tail = url.slice(-8);
    return `${protocol}//${hostname}/…${tail}`;
  } catch {
    return `…${url.slice(-12)}`;
  }
}

function useWebhookPanel() {
  const [panelState, setPanelState] = useState<PanelState>('idle');
  const [url, setUrl] = useState('');
  const [urlError, setUrlError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<'ok' | 'error' | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const [usage, setUsage] = useState<UsageResponse | undefined>(undefined);

  useEffect(() => {
    invoke<string>('get_scheduler_credential', { provider: 'webhook' })
      .then((p) => { setPreview(maskWebhookUrl(p)); setPanelState('configured'); })
      .catch(() => { setPanelState('idle'); });
    invoke<UsageResponse>('get_scheduler_usage', { provider: 'webhook' })
      .then(setUsage)
      .catch(() => { /* non-critical */ });
  }, []);

  function handleUrlChange(v: string) { setUrl(v); setUrlError(validateUrl(v)); }

  async function handleSave() {
    const err = validateUrl(url);
    if (err) { setUrlError(err); return; }
    if (!url) return;
    setSaving(true);
    setSaveError(null);
    try {
      await invoke('save_scheduler_credential', { provider: 'webhook', apiKey: url });
      setPreview(maskWebhookUrl(url)); setUrl(''); setUrlError(null); setPanelState('configured');
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : 'Failed to save credential');
    } finally { setSaving(false); }
  }

  async function handleTest() {
    setTesting(true); setTestResult(null);
    try { await invoke('test_scheduler', { provider: 'webhook' }); setTestResult('ok'); }
    catch (e) { setTestResult('error'); setTestError(e instanceof Error ? e.message : 'Test failed'); }
    finally { setTesting(false); }
  }

  async function handleRemove() {
    try {
      await invoke('delete_scheduler_credential', { provider: 'webhook' });
      setPreview(null); setPanelState('idle'); setTestResult(null);
    } catch { /* silent */ }
  }

  function handleCancel() {
    setPanelState(preview ? 'configured' : 'idle'); setUrl(''); setUrlError(null);
  }

  return { panelState, setPanelState, url, urlError, saveError, preview, saving, testing, testResult, testError, usage, handleUrlChange, handleSave, handleTest, handleRemove, handleCancel };
}

export default function WebhookPanel() {
  const { panelState, setPanelState, url, urlError, saveError, preview, saving, testing, testResult, testError, usage, handleUrlChange, handleSave, handleTest, handleRemove, handleCancel } = useWebhookPanel();

  return (
    <div className="box p-4">
      <div className="is-flex is-align-items-center mb-3" style={{ gap: '0.75rem' }}>
        <h3 className="has-text-weight-medium is-size-7">Webhook</h3>
        <UsageBadge usage={usage} />
      </div>
      {panelState === 'idle' && <IdleView onStartAdd={() => setPanelState('adding')} />}
      {panelState === 'adding' && (
        <AddingForm url={url} urlError={urlError} saveError={saveError} saving={saving} onUrlChange={handleUrlChange}
          onSave={handleSave} onCancel={handleCancel} />
      )}
      {panelState === 'configured' && preview && (
        <ConfiguredView preview={preview} testing={testing} testResult={testResult} testError={testError}
          onTest={handleTest} onChange={() => setPanelState('adding')} onRemove={handleRemove} />
      )}
    </div>
  );
}
