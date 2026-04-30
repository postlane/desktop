// SPDX-License-Identifier: BUSL-1.1

import { useCallback, type KeyboardEvent } from 'react';
import type { Platform } from '../types';

export function usePostCardKeyboard(
  isFocused: boolean, isFailed: boolean, platforms: Platform[],
  approve: () => void, dismiss: () => void, retry: () => void,
  setActiveTab: (_p: Platform) => void, setExpanded: (_fn: (_v: boolean) => boolean) => void,
): (_e: KeyboardEvent<HTMLElement>) => void {
  return useCallback((e: KeyboardEvent<HTMLElement>) => {
    if (!isFocused) return;
    const key = e.key.toLowerCase();
    const numIdx = parseInt(key, 10) - 1;
    if (numIdx >= 0 && numIdx < Math.min(5, platforms.length)) { setActiveTab(platforms[numIdx]); return; }
    const actions: Partial<Record<string, () => void>> = {
      a: () => { e.preventDefault(); approve(); },
      d: () => { e.preventDefault(); dismiss(); },
      e: () => { e.preventDefault(); setExpanded((v) => !v); },
      r: () => { if (isFailed) { e.preventDefault(); retry(); } },
      escape: () => { e.preventDefault(); setExpanded(() => false); },
    };
    actions[key]?.();
  }, [isFocused, isFailed, platforms, approve, dismiss, retry, setActiveTab, setExpanded]);
}
