// SPDX-License-Identifier: BUSL-1.1

import { useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { ViewSelection } from '../types';

const PERSIST_DEBOUNCE_MS = 300;

export function useNavPersistence(): (_ids: Set<string>, _view: ViewSelection) => void {
  const persistTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const scheduleWrite = useCallback((ids: Set<string>, view: ViewSelection) => {
    if (persistTimerRef.current) clearTimeout(persistTimerRef.current);
    persistTimerRef.current = setTimeout(async () => {
      try {
        const win = getCurrentWindow();
        const [size, pos] = await Promise.all([win.outerSize(), win.outerPosition()]);
        await invoke<void>('save_app_state_command', {
          state: { version: 1, window: { width: size.width, height: size.height, x: pos.x, y: pos.y }, nav: { last_view: view.view, last_repo_id: view.repoId, last_section: view.section, expanded_repos: [...ids] } },
        });
      } catch (e) { console.error('Failed to persist nav state:', e); }
    }, PERSIST_DEBOUNCE_MS);
  }, []);

  return scheduleWrite;
}
