// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

type PanelState = 'idle' | 'adding' | 'configured';

function SubstackWarnings() {
  return (
    <div className="is-size-7 has-text-warning-dark" style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
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
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <SubstackWarnings />
      <button className="button is-outlined is-small" onClick={onStartAdd}>+ Add</button>
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
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <SubstackWarnings />
      <p className="is-size-7 has-text-grey">
        Paste your Substack session cookie (<code>connect.sid</code>) here. Find it in your browser&apos;s
        DevTools → Application → Cookies → substack.com after logging in.
      </p>
      <textarea value={cookie} onChange={(e) => onCookieChange(e.target.value)}
        placeholder="connect.sid cookie value" rows={3} className="textarea is-small" />
      {saveError && <p className="is-size-7 has-text-danger">{saveError}</p>}
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
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <SubstackWarnings />
      <div className="is-flex is-align-items-center is-justify-content-space-between" style={{ gap: '1rem' }}>
        <span className="is-size-7 has-text-grey">{preview}</span>
        <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
          {testResult === 'ok' && <span className="is-size-7 has-text-success">✓</span>}
          {testResult === 'error' && <span className="is-size-7 has-text-danger">{testError}</span>}
          <button className="button is-outlined is-small" onClick={onTest} disabled={testing}>Test</button>
          <button className="button is-outlined is-small" onClick={onChange}>Change</button>
          <button className="button is-outlined is-small" onClick={onRemove}>Remove</button>
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
    <div className="box p-4">
      <h3 className="has-text-weight-medium is-size-7 mb-3">Substack Notes</h3>
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
