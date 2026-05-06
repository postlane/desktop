// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { confirm } from '@tauri-apps/plugin-dialog';
import { useTimezone, utcIsoToDatetimeLocal, localDatetimeToUtcIso } from '../TimezoneContext';

const INPUT_CLASS = 'rounded-lg border border-zinc-300 bg-white px-2 py-1 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500';

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
      <div className="mt-3 flex items-center gap-2">
        <span className="text-sm text-zinc-500 dark:text-zinc-400">Scheduled</span>
        <button onClick={() => setInputVisible(true)} className="text-xs text-blue-600 hover:underline dark:text-blue-400">
          + Add time
        </button>
      </div>
    );
  }

  return (
    <div className="mt-3 space-y-1">
      <div className="flex items-center gap-2">
        <span className="shrink-0 text-sm text-zinc-500 dark:text-zinc-400">Scheduled</span>
        <input
          type="datetime-local"
          aria-label="Scheduled time"
          value={inputValue}
          onChange={(e) => { setInputValue(e.target.value); void handleChange(e.target.value); }}
          className={INPUT_CLASS}
        />
        <button aria-label="Clear schedule" onClick={handleClear} className="text-xs text-zinc-400 hover:text-red-500">Clear</button>
      </div>
      {error && <p className="text-xs text-red-600 dark:text-red-400">{error}</p>}
    </div>
  );
}
