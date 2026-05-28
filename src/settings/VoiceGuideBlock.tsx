// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '../ipc/invoke';
import { VoiceGuideForm, VoiceGuideFields, EMPTY_FIELDS, buildVoiceGuide } from './VoiceGuideForm';

interface Props {
  projectId: string;
  projectName: string;
  isOwner: boolean;
}

interface SyncStatus {
  synced: string[];
  registered: number;
}

type SyncState =
  | null
  | { kind: 'synced'; count: number }
  | { kind: 'no-repos' }
  | { kind: 'paths-missing' };

function useVoiceGuideFields(projectId: string) {
  const [fields, setFields] = useState<VoiceGuideFields>(EMPTY_FIELDS);
  const [loadedFields, setLoadedFields] = useState<VoiceGuideFields>(EMPTY_FIELDS);
  const [loadError, setLoadError] = useState(false);
  const [syncState, setSyncState] = useState<SyncState>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
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
    setSaveError(null);
    try {
      const status = await invoke<SyncStatus>('save_project_voice_guide', {
        projectId,
        voiceGuide: buildVoiceGuide(current, projectName),
        voiceGuideFields: current,
      });
      setLoadedFields(current);
      const synced = status?.synced ?? [];
      const registered = status?.registered ?? 0;
      const next: SyncState =
        synced.length > 0 ? { kind: 'synced', count: synced.length }
        : registered > 0 ? { kind: 'paths-missing' }
        : { kind: 'no-repos' };
      setSyncState(next);
      if (timerRef.current) clearTimeout(timerRef.current);
      // paths-missing requires user action (reconnect repo paths) — do not auto-dismiss.
      // synced and no-repos are purely informational and can safely disappear.
      if (next.kind !== 'paths-missing') {
        timerRef.current = setTimeout(() => setSyncState(null), 2000);
      }
    } catch {
      setSaveError('Failed to save voice guide. Check your connection and try again.');
    } finally {
      setSaveLoading(false);
    }
  }, [projectId]);

  const isDirty = JSON.stringify(fields) !== JSON.stringify(loadedFields);
  return { fields, setFields, loadError, syncState, setSyncState, saveError, saveLoading, load, save, isDirty };
}

export default function VoiceGuideBlock({ projectId, projectName, isOwner }: Props) {
  const { fields, setFields, loadError, syncState, setSyncState, saveError, saveLoading, load, save, isDirty } = useVoiceGuideFields(projectId);

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
          {saveError !== null && (
            <div className="notification is-danger is-light py-2 px-3 mt-2">
              <p className="is-size-7">{saveError}</p>
            </div>
          )}
          <div className="is-flex is-align-items-center mt-2" style={{ gap: '0.5rem' }}>
            {syncState !== null && (
              syncState.kind === 'synced'
                ? <p className="is-size-7 has-text-success">Voice guide saved and synced to {syncState.count} repo(s).</p>
                : syncState.kind === 'paths-missing'
                ? <span className="is-flex is-align-items-center is-size-7 has-text-warning" style={{ gap: '0.25rem' }}>
                    <span>Voice guide saved, but your connected repo paths could not be found on disk. Check Repositories in Settings.</span>
                    <button type="button" className="button is-ghost is-small" aria-label="Dismiss" onClick={() => setSyncState(null)}>✕</button>
                  </span>
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
