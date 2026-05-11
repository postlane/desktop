// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '../ipc/invoke';

// ── Constants ─────────────────────────────────────────────────────────────────

const CHAR_LIMIT = 5000;

const TEMPLATES: { label: string; text: string }[] = [
  {
    label: 'Professional & direct',
    text: 'Write posts that are clear, confident, and informative. Lead with the key point. No filler phrases, no hedging. Suitable for a professional audience that values directness over warmth.',
  },
  {
    label: 'Conversational & approachable',
    text: 'Write posts that feel human and easy to read. Use short sentences and plain language. Sound like a thoughtful colleague sharing something useful, not a press release.',
  },
  {
    label: 'Technical & precise',
    text: 'Write posts for an engineering audience. Use accurate technical vocabulary without over-explaining basics. Include specific version numbers, command names, or metrics where relevant. Avoid superlatives — let the facts speak.',
  },
];

// ── Types ─────────────────────────────────────────────────────────────────────

interface Props {
  projectId: string;
  isOwner: boolean;
}

// ── Sub-components ────────────────────────────────────────────────────────────

function TemplateConfirm({ onReplace, onCancel }: { onReplace: () => void; onCancel: () => void }) {
  return (
    <div className="notification is-warning is-light mt-2 py-2 px-3">
      <p className="is-size-7">Replace your current text with this template?</p>
      <div className="is-flex mt-2" style={{ gap: '0.5rem' }}>
        <button className="button is-small is-warning" onClick={onReplace}>Replace</button>
        <button className="button is-small" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}

// ── Hook ──────────────────────────────────────────────────────────────────────

function useVoiceGuide(projectId: string) {
  const [text, setText] = useState('');
  const [loadedValue, setLoadedValue] = useState('');
  const [loadError, setLoadError] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [saveLoading, setSaveLoading] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const load = useCallback(() => {
    setLoadError(false);
    invoke<string>('get_project_voice_guide', { projectId })
      .then((v) => { setText(v ?? ''); setLoadedValue(v ?? ''); })
      .catch(() => setLoadError(true));
  }, [projectId]);

  useEffect(() => { load(); }, [load]);

  const save = useCallback(async (currentText: string) => {
    setSaveLoading(true);
    try {
      await invoke('save_project_voice_guide', { projectId, voiceGuide: currentText });
      setLoadedValue(currentText);
      setSaveSuccess(true);
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => setSaveSuccess(false), 2000);
    } finally {
      setSaveLoading(false);
    }
  }, [projectId]);

  return { text, setText, loadedValue, loadError, saveSuccess, saveLoading, load, save };
}

// ── Main component ────────────────────────────────────────────────────────────

export default function VoiceGuideBlock({ projectId, isOwner }: Props) {
  const { text, setText, loadedValue, loadError, saveSuccess, saveLoading, load, save } = useVoiceGuide(projectId);
  const [showTemplates, setShowTemplates] = useState(false);
  const [pendingTemplate, setPendingTemplate] = useState<string | null>(null);
  const charCount = text.length;
  const isOverLimit = charCount > CHAR_LIMIT;
  const isDirty = text !== loadedValue;
  const saveDisabled = !isDirty || isOverLimit || saveLoading || loadError;
  const saveTitle = isOverLimit ? `Voice guide cannot exceed ${CHAR_LIMIT} characters` : undefined;

  function handleTemplateClick(templateText: string) {
    setShowTemplates(false);
    if (isDirty) { setPendingTemplate(templateText); } else { setText(templateText); }
  }

  return (
    <div>
      <p className="is-size-6 has-text-weight-medium mb-3">Voice guide</p>
      {loadError && (
        <div className="notification is-danger is-light py-2 px-3 mb-2">
          <p className="is-size-7">Failed to load voice guide.</p>
          <button className="button is-small mt-2" onClick={load}>Retry</button>
        </div>
      )}
      {isOwner
        ? <textarea className="textarea is-size-7" value={text} onChange={(e) => setText(e.target.value)} disabled={loadError} rows={6} />
        : <pre className="is-size-7" style={{ whiteSpace: 'pre-wrap' }}>{text}</pre>}
      <div className="is-flex is-align-items-center mt-2" style={{ gap: '0.5rem' }}>
        <span data-testid="char-count" className={`is-size-7 ${isOverLimit ? 'has-text-danger' : 'has-text-grey'}`}>{charCount} / {CHAR_LIMIT}</span>
        {isOwner && (
          <>
            <div style={{ position: 'relative' }}>
              <button className="button is-small is-ghost" onClick={() => setShowTemplates((v) => !v)}>Templates</button>
              {showTemplates && (
                <div className="dropdown-content" style={{ position: 'absolute', top: '100%', left: 0, zIndex: 10, background: 'white', border: '1px solid var(--bulma-border)', borderRadius: 4, minWidth: '14rem' }}>
                  {TEMPLATES.map((t) => (
                    <button key={t.label} className="dropdown-item button is-ghost is-fullwidth" style={{ textAlign: 'left' }} onClick={() => handleTemplateClick(t.text)}>{t.label}</button>
                  ))}
                </div>
              )}
            </div>
            <button className="button is-small is-primary ml-auto" onClick={() => save(text)} disabled={saveDisabled} title={saveTitle}>Save</button>
          </>
        )}
      </div>
      {pendingTemplate && (
        <TemplateConfirm onReplace={() => { setText(pendingTemplate); setPendingTemplate(null); }} onCancel={() => setPendingTemplate(null)} />
      )}
      {saveSuccess && <p className="is-size-7 has-text-success mt-1">Voice guide saved.</p>}
    </div>
  );
}
