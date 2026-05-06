// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { confirm } from '@tauri-apps/plugin-dialog';
import { VOICE_GUIDE_TEMPLATE } from '../wizard/ModalVoiceGuide';

const PLACEHOLDER = 'No voice guide set. Add one to tell the LLM how to write posts for this project.';
const TA_CLASS = 'w-full rounded-lg border border-zinc-300 bg-white px-3 py-2 font-mono text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 disabled:opacity-50';

type SaveState = 'idle' | 'saving' | 'saved' | 'error';

function useVoiceGuide(projectId: string) {
  const [text, setText] = useState('');
  const [loading, setLoading] = useState(true);
  const [saveState, setSaveState] = useState<SaveState>('idle');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string | null>('get_project_voice_guide', { projectId })
      .then((v) => setText(v ?? ''))
      .catch((e) => setError(e instanceof Error ? e.message : 'Failed to load voice guide'))
      .finally(() => setLoading(false));
  }, [projectId]);

  useEffect(() => {
    if (saveState !== 'saved') return;
    const timer = setTimeout(() => setSaveState('idle'), 2000);
    return () => clearTimeout(timer);
  }, [saveState]);

  async function handleSave() {
    if (text === '') {
      const ok = await confirm('Save an empty voice guide? This will clear the current guide.', { title: 'Clear voice guide', kind: 'warning' });
      if (!ok) return;
    }
    setSaveState('saving'); setError(null);
    try {
      await invoke('save_project_voice_guide', { projectId, voiceGuide: text });
      setSaveState('saved');
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save');
      setSaveState('error');
    }
  }

  return { text, setText, loading, saveState, error, handleSave };
}

export function VoiceGuideSection({ projectId }: { projectId: string }) {
  const { text, setText, loading, saveState, error, handleSave } = useVoiceGuide(projectId);
  return (
    <div className="mt-6 space-y-2">
      <h3 className="text-sm font-medium text-zinc-700 dark:text-zinc-300">Voice guide</h3>
      <textarea
        aria-label="Voice guide"
        value={text}
        onChange={(e) => setText(e.target.value)}
        disabled={loading}
        rows={8}
        placeholder={PLACEHOLDER}
        className={TA_CLASS}
      />
      <p className="text-xs text-zinc-500 dark:text-zinc-400">
        Saved to <code>.postlane/voice-guide.md</code> in your repo.
      </p>
      <div className="flex items-center gap-3">
        {text === '' && !loading && (
          <button onClick={() => setText(VOICE_GUIDE_TEMPLATE)} className="text-xs text-blue-600 hover:underline dark:text-blue-400">
            Start from template
          </button>
        )}
        <button aria-label="Save voice guide" onClick={handleSave} disabled={saveState === 'saving'} className="rounded-lg bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700 disabled:opacity-50">
          {saveState === 'saving' ? 'Saving…' : 'Save'}
        </button>
        {saveState === 'saved' && <span className="text-xs text-green-600">✓ Saved</span>}
        {error && <p className="text-xs text-red-600">{error}</p>}
      </div>
    </div>
  );
}
