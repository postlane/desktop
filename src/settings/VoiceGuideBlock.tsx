// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '../ipc/invoke';
import { VoiceGuideForm, VoiceGuideFields, EMPTY_FIELDS, buildVoiceGuide } from './VoiceGuideForm';

interface Props {
  projectId: string;
  projectName: string;
  isOwner: boolean;
}

function useVoiceGuideFields(projectId: string) {
  const [fields, setFields] = useState<VoiceGuideFields>(EMPTY_FIELDS);
  const [loadedFields, setLoadedFields] = useState<VoiceGuideFields>(EMPTY_FIELDS);
  const [loadError, setLoadError] = useState(false);
  const [syncedCount, setSyncedCount] = useState<number | null>(null);
  const [saveLoading, setSaveLoading] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const load = useCallback(() => {
    setLoadError(false);
    invoke<Partial<VoiceGuideFields> | null>('get_voice_guide_fields', { projectId })
      .then((data) => {
        if (data && typeof data === 'object') {
          const loaded = { ...EMPTY_FIELDS, ...data };
          setFields(loaded);
          setLoadedFields(loaded);
        }
      })
      .catch(() => setLoadError(true));
  }, [projectId]);

  useEffect(() => { load(); }, [load]);

  const save = useCallback(async (current: VoiceGuideFields, projectName: string) => {
    setSaveLoading(true);
    try {
      const synced = await invoke<string[]>('save_project_voice_guide', {
        projectId,
        voiceGuide: buildVoiceGuide(current, projectName),
        voiceGuideFields: current,
      });
      setLoadedFields(current);
      setSyncedCount((synced ?? []).length);
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => setSyncedCount(null), 2000);
    } finally {
      setSaveLoading(false);
    }
  }, [projectId]);

  const isDirty = JSON.stringify(fields) !== JSON.stringify(loadedFields);
  return { fields, setFields, loadError, syncedCount, saveLoading, load, save, isDirty };
}

export default function VoiceGuideBlock({ projectId, projectName, isOwner }: Props) {
  const { fields, setFields, loadError, syncedCount, saveLoading, load, save, isDirty } = useVoiceGuideFields(projectId);

  function handleChange(key: keyof VoiceGuideFields, value: string) {
    setFields((prev) => ({ ...prev, [key]: value }));
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
      {isOwner ? (
        <>
          <VoiceGuideForm fields={fields} onChange={handleChange} onApplyTemplate={setFields} />
          <div className="is-flex is-align-items-center mt-2" style={{ gap: '0.5rem' }}>
            {syncedCount !== null && (
              syncedCount > 0
                ? <p className="is-size-7 has-text-success">Voice guide saved and synced to {syncedCount} repo(s).</p>
                : <p className="is-size-7 has-text-success">Voice guide saved. Connect a repository to sync it there.</p>
            )}
            <button className="button is-small is-primary ml-auto"
              onClick={() => save(fields, projectName)} disabled={!isDirty || saveLoading}>
              {saveLoading ? 'Saving…' : 'Save'}
            </button>
          </div>
        </>
      ) : (
        <pre className="is-size-7" style={{ whiteSpace: 'pre-wrap' }}>{buildVoiceGuide(fields, projectName)}</pre>
      )}
    </div>
  );
}
