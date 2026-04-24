// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/catalyst/button';

type PanelState = 'idle' | 'adding' | 'configured';

interface IdleViewProps {
  onStartAdd: () => void;
}

function IdleView({ onStartAdd }: IdleViewProps) {
  return <Button outline onClick={onStartAdd}>+ Add</Button>;
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
    <div className="space-y-3">
      <p className="text-xs text-zinc-500 dark:text-zinc-400">
        Enter a webhook URL. Postlane will POST the scheduled content as JSON to this endpoint.
      </p>
      <div className="space-y-1">
        <input
          type="url"
          value={url}
          onChange={(e) => onUrlChange(e.target.value)}
          placeholder="https://hooks.example.com/webhook"
          className="w-full rounded-lg border border-zinc-300 px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
        />
        {urlError && <p className="text-xs text-red-600">{urlError}</p>}
        {saveError && <p className="text-xs text-red-600">{saveError}</p>}
      </div>
      <div className="flex gap-2">
        <Button onClick={onSave} disabled={saving}>{saving ? 'Saving…' : 'Save'}</Button>
        <Button plain onClick={onCancel}>Cancel</Button>
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
    <div className="flex items-center justify-between gap-4">
      <span className="text-xs text-zinc-500 truncate">{preview}</span>
      <div className="flex items-center gap-2 shrink-0">
        {testResult === 'ok' && <span className="text-xs text-green-600">✓</span>}
        {testResult === 'error' && <span className="text-xs text-red-600">{testError}</span>}
        <Button outline onClick={onTest} disabled={testing}>Test</Button>
        <Button outline onClick={onChange}>Change</Button>
        <Button outline onClick={onRemove}>Remove</Button>
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

  useEffect(() => {
    invoke<string>('get_scheduler_credential', { provider: 'webhook' })
      .then((p) => { setPreview(maskWebhookUrl(p)); setPanelState('configured'); })
      .catch(() => { setPanelState('idle'); });
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

  return { panelState, setPanelState, url, urlError, saveError, preview, saving, testing, testResult, testError, handleUrlChange, handleSave, handleTest, handleRemove, handleCancel };
}

export default function WebhookPanel() {
  const { panelState, setPanelState, url, urlError, saveError, preview, saving, testing, testResult, testError, handleUrlChange, handleSave, handleTest, handleRemove, handleCancel } = useWebhookPanel();

  return (
    <div className="rounded-lg border border-zinc-200 p-4 dark:border-zinc-700">
      <h3 className="mb-3 text-sm font-medium text-zinc-900 dark:text-zinc-100">Webhook</h3>
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
