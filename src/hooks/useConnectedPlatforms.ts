// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { invoke } from '../ipc/invoke';
import { listen } from '@tauri-apps/api/event';

/**
 * Returns connected platform slugs keyed by repo ID.
 * Fetched once per unique repoId set at queue load time (§21.11.3).
 * Refreshed when the `platform-connected` Tauri event fires (§21.11.15).
 */
export function useConnectedPlatforms(repoIds: string[]): Record<string, string[]> {
  const [result, setResult] = useState<Record<string, string[]>>({});
  const repoIdsRef = useRef(repoIds);
  repoIdsRef.current = repoIds;

  const stableKey = useMemo(() => [...repoIds].sort().join('\0'), [repoIds]);

  const refresh = useCallback(() => {
    const ids = repoIdsRef.current;
    if (ids.length === 0) return;
    Promise.all(
      ids.map(async (repoId) => {
        const raw = await invoke<string[] | null>('list_connected_platforms', { repoId }).catch(
          () => null,
        );
        return [repoId, Array.isArray(raw) ? raw : []] as const;
      }),
    ).then((entries) => setResult(Object.fromEntries(entries)));
  }, []);

  useEffect(() => {
    refresh();
  }, [stableKey, refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;
    listen('platform-connected', () => refresh())
      .then((fn) => {
        if (mounted) { unlisten = fn; } else { fn(); }
      })
      .catch(console.error);
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [refresh]);

  return result;
}
