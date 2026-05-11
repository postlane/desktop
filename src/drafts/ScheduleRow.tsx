// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '../ipc/invoke';
import { confirm } from '@tauri-apps/plugin-dialog';
import { useTimezone, utcIsoToDatetimeLocal, localDatetimeToUtcIso } from '../TimezoneContext';

interface Props {
  repoPath: string;
  postFolder: string;
  schedule: string | null;
  onScheduleChange: (_s: string | null) => void;
}

function useScheduleRow({ repoPath, postFolder, schedule, onScheduleChange }: Props) {
  const tz = useTimezone();
  const [inputVisible, setInputVisible] = useState(!!schedule);
  const [inputValue, setInputValue] = useState(() => utcIsoToDatetimeLocal(schedule ?? '', tz));
  const [error, setError] = useState<string | null>(null);
  const inflightIdRef = useRef(0);

  useEffect(() => {
    setInputValue(utcIsoToDatetimeLocal(schedule ?? '', tz));
    if (schedule) setInputVisible(true);
  }, [schedule, tz]);

  async function handleChange(localValue: string) {
    const utcIso = localDatetimeToUtcIso(localValue, tz);
    const prev = schedule;
    const myId = ++inflightIdRef.current;
    setError(null);
    onScheduleChange(utcIso);
    try {
      await invoke('update_post_schedule', { repoPath, postFolder, schedule: utcIso, timezone: tz });
    } catch (e) {
      if (inflightIdRef.current === myId) {
        onScheduleChange(prev);
        setError(e instanceof Error ? e.message : 'Failed to update schedule');
      }
    }
  }

  async function handleClear() {
    const ok = await confirm('Clear the scheduled time?', { title: 'Clear schedule', kind: 'warning' });
    if (!ok) return;
    const prev = schedule;
    setError(null);
    onScheduleChange(null);
    setInputVisible(false);
    try {
      await invoke('update_post_schedule', { repoPath, postFolder, schedule: null, timezone: tz });
    } catch (e) {
      onScheduleChange(prev);
      setInputVisible(true);
      setError(e instanceof Error ? e.message : 'Failed to clear schedule');
    }
  }

  return { inputVisible, setInputVisible, inputValue, setInputValue, error, handleChange, handleClear };
}

export function ScheduleRow(props: Props) {
  const { inputVisible, setInputVisible, inputValue, setInputValue, error, handleChange, handleClear } = useScheduleRow(props);

  if (!inputVisible) {
    return (
      <div className="mt-3 is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <span className="is-size-7 has-text-grey">Scheduled</span>
        <button onClick={() => setInputVisible(true)} className="button is-ghost is-small has-text-link">
          + Add time
        </button>
      </div>
    );
  }

  return (
    <div className="mt-3" style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <span className="is-size-7 has-text-grey" style={{ flexShrink: 0 }}>Scheduled</span>
        <input
          type="datetime-local"
          aria-label="Scheduled time"
          value={inputValue}
          onChange={(e) => { setInputValue(e.target.value); void handleChange(e.target.value); }}
          className="input is-small"
        />
        <button aria-label="Clear schedule" onClick={handleClear} className="button is-ghost is-small has-text-grey-light">Clear</button>
      </div>
      {error && <p className="has-text-danger is-size-7">{error}</p>}
    </div>
  );
}
