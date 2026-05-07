// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { confirm } from '@tauri-apps/plugin-dialog';

const VOICE_GUIDE_TEMPLATE = `Write in a clear, direct tone. No marketing language.
Focus on what changed and why it matters to the reader.
Keep posts concise — one idea per post.
Use plain language; avoid jargon unless your audience expects it.`;

const PLACEHOLDER = 'No voice guide set. Add one to tell the LLM how to write posts for this project.';

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
    <div className="mt-5" style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
      <h3 className="has-text-weight-medium is-size-7">Voice guide</h3>
      <textarea aria-label="Voice guide" value={text} onChange={(e) => setText(e.target.value)}
        disabled={loading} rows={8} placeholder={PLACEHOLDER}
        className="textarea is-small is-family-monospace" style={{ opacity: loading ? 0.5 : 1 }} />
      <p className="is-size-7 has-text-grey">
        Saved to <code>.postlane/voice-guide.md</code> in your repo.
      </p>
      <div className="is-flex is-align-items-center" style={{ gap: '0.75rem' }}>
        {text === '' && !loading && (
          <button onClick={() => setText(VOICE_GUIDE_TEMPLATE)} className="button is-ghost is-small has-text-link">
            Start from template
          </button>
        )}
        <button aria-label="Save voice guide" onClick={handleSave} disabled={saveState === 'saving'}
          className="button is-primary is-small">
          {saveState === 'saving' ? 'Saving…' : 'Save'}
        </button>
        {saveState === 'saved' && <span className="is-size-7 has-text-success">✓ Saved</span>}
        {error && <p className="is-size-7 has-text-danger">{error}</p>}
      </div>
    </div>
  );
}
