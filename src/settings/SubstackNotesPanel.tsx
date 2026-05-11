// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { useCredentialPanel } from './useCredentialPanel';

const MASK_SUBSTACK = (raw: string) => `••••${raw.slice(-4)}`;

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
  const [cookie, setCookie] = useState('');
  const base = useCredentialPanel({ provider: 'substack_notes', maskCredential: MASK_SUBSTACK });

  async function handleSave() {
    const ok = await base.saveCredential(cookie);
    if (ok) setCookie('');
  }

  function handleCancel() {
    base.setPanelState(base.preview ? 'configured' : 'idle');
    setCookie('');
    base.setSaveError(null);
  }

  return { ...base, cookie, setCookie, handleSave, handleCancel };
}

export default function SubstackNotesPanel() {
  const { panelState, setPanelState, cookie, setCookie, preview, saving, saveError, removeError, testing, testResult, testError, handleSave, handleTest, handleRemove, handleCancel } = useSubstackPanel();

  return (
    <div className="box p-4">
      <h3 className="has-text-weight-medium is-size-7 mb-3">Substack Notes</h3>
      {removeError && <p role="alert" className="is-size-7 has-text-danger mb-2">{removeError}</p>}
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
