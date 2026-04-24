// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/catalyst/button';

type PanelState = 'idle' | 'adding' | 'configured';

interface WarningsProps {}

function SubstackWarnings(_: WarningsProps) {
  return (
    <div className="space-y-1.5 text-xs text-amber-700 dark:text-amber-400">
      <p>Your session expires when you sign out of Substack. If posting fails, re-enter your credentials here.</p>
      <p>Substack Notes always post immediately — scheduled times are not supported.</p>
    </div>
  );
}

interface IdleFormProps {
  onStartAdd: () => void;
}

function IdleView({ onStartAdd }: IdleFormProps) {
  return (
    <div className="space-y-3">
      <SubstackWarnings />
      <Button outline onClick={onStartAdd}>+ Add</Button>
    </div>
  );
}

interface AddingFormProps {
  cookie: string;
  saving: boolean;
  saveError: string | null;
  onCookieChange: (_v: string) => void;
  onSave: () => void;
  onCancel: () => void;
}

function AddingForm({ cookie, saving, saveError, onCookieChange, onSave, onCancel }: AddingFormProps) {
  return (
    <div className="space-y-3">
      <SubstackWarnings />
      <p className="text-xs text-zinc-500 dark:text-zinc-400">
        Paste your Substack session cookie (<code>connect.sid</code>) here. Find it in your browser&apos;s
        DevTools → Application → Cookies → substack.com after logging in.
      </p>
      <textarea
        value={cookie}
        onChange={(e) => onCookieChange(e.target.value)}
        placeholder="connect.sid cookie value"
        rows={3}
        className="w-full rounded-lg border border-zinc-300 px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
      />
      {saveError && <p className="text-xs text-red-600">{saveError}</p>}
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
    <div className="space-y-3">
      <SubstackWarnings />
      <div className="flex items-center justify-between gap-4">
        <span className="text-xs text-zinc-500">{preview}</span>
        <div className="flex items-center gap-2">
          {testResult === 'ok' && <span className="text-xs text-green-600">✓</span>}
          {testResult === 'error' && <span className="text-xs text-red-600">{testError}</span>}
          <Button outline onClick={onTest} disabled={testing}>Test</Button>
          <Button outline onClick={onChange}>Change</Button>
          <Button outline onClick={onRemove}>Remove</Button>
        </div>
      </div>
    </div>
  );
}

function useSubstackPanel() {
  const [panelState, setPanelState] = useState<PanelState>('idle');
  const [cookie, setCookie] = useState('');
  const [preview, setPreview] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<'ok' | 'error' | null>(null);
  const [testError, setTestError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>('get_scheduler_credential', { provider: 'substack_notes' })
      .then((p) => { setPreview(p); setPanelState('configured'); })
      .catch(() => { setPanelState('idle'); });
  }, []);

  async function handleSave() {
    if (!cookie) return;
    setSaving(true); setSaveError(null);
    try {
      await invoke('save_scheduler_credential', { provider: 'substack_notes', apiKey: cookie });
      setPreview(`••••${cookie.slice(-4)}`); setCookie(''); setPanelState('configured');
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : 'Failed to save credential');
    } finally { setSaving(false); }
  }

  async function handleTest() {
    setTesting(true); setTestResult(null);
    try { await invoke('test_scheduler', { provider: 'substack_notes' }); setTestResult('ok'); }
    catch (e) { setTestResult('error'); setTestError(e instanceof Error ? e.message : 'Test failed'); }
    finally { setTesting(false); }
  }

  async function handleRemove() {
    try {
      await invoke('delete_scheduler_credential', { provider: 'substack_notes' });
      setPreview(null); setPanelState('idle'); setTestResult(null);
    } catch { /* silent */ }
  }

  function handleCancel() {
    setPanelState(preview ? 'configured' : 'idle'); setCookie(''); setSaveError(null);
  }

  return { panelState, setPanelState, cookie, setCookie, preview, saving, saveError, testing, testResult, testError, handleSave, handleTest, handleRemove, handleCancel };
}

export default function SubstackNotesPanel() {
  const { panelState, setPanelState, cookie, setCookie, preview, saving, saveError, testing, testResult, testError, handleSave, handleTest, handleRemove, handleCancel } = useSubstackPanel();

  return (
    <div className="rounded-lg border border-zinc-200 p-4 dark:border-zinc-700">
      <h3 className="mb-3 text-sm font-medium text-zinc-900 dark:text-zinc-100">Substack Notes</h3>
      {panelState === 'idle' && <IdleView onStartAdd={() => setPanelState('adding')} />}
      {panelState === 'adding' && (
        <AddingForm cookie={cookie} saving={saving} saveError={saveError} onCookieChange={setCookie}
          onSave={handleSave} onCancel={handleCancel} />
      )}
      {panelState === 'configured' && preview && (
        <ConfiguredView preview={preview} testing={testing} testResult={testResult} testError={testError}
          onTest={handleTest} onChange={() => setPanelState('adding')} onRemove={handleRemove} />
      )}
    </div>
  );
}
