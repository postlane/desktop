// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';

export type PanelState = 'idle' | 'adding' | 'configured';

interface Options {
  provider: string;
  maskCredential: (raw: string) => string;
}

export function useCredentialPanel({ provider, maskCredential }: Options) {
  const [panelState, setPanelState] = useState<PanelState>('idle');
  const [preview, setPreview] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<'ok' | 'error' | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const [removeError, setRemoveError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>('get_scheduler_credential', { provider })
      .then((raw) => { setPreview(maskCredential(raw)); setPanelState('configured'); })
      .catch(() => { setPanelState('idle'); });
  }, [provider, maskCredential]);

  async function saveCredential(value: string): Promise<boolean> {
    if (!value) return false;
    setSaving(true); setSaveError(null);
    try {
      await invoke('save_scheduler_credential', { provider, apiKey: value });
      setPreview(maskCredential(value)); setPanelState('configured');
      return true;
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : 'Failed to save credential');
      return false;
    } finally { setSaving(false); }
  }

  async function handleTest() {
    setTesting(true); setTestResult(null);
    try { await invoke('test_scheduler', { provider }); setTestResult('ok'); }
    catch (e) { setTestResult('error'); setTestError(e instanceof Error ? e.message : 'Test failed'); }
    finally { setTesting(false); }
  }

  async function handleRemove() {
    setRemoveError(null);
    try {
      await invoke('delete_scheduler_credential', { provider });
      setPreview(null); setPanelState('idle'); setTestResult(null);
    } catch (e) {
      setRemoveError(e instanceof Error ? e.message : 'Failed to remove credential');
    }
  }

  return {
    panelState, setPanelState, preview, saving, saveError, setSaveError,
    removeError, testing, testResult, testError, saveCredential, handleTest, handleRemove,
  };
}
