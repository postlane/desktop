// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { MetaChangedPayload } from '../types';

export function useMetaChangedListener(onRefresh: () => void): Map<string, Date> {
  const [lastWatcherEvent, setLastWatcherEvent] = useState<Map<string, Date>>(new Map());

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;
    listen<MetaChangedPayload>('meta-changed', (event) => {
      setLastWatcherEvent((prev) => { const next = new Map(prev); next.set(event.payload.repo_id, new Date()); return next; });
      onRefresh();
    })
      .then((fn) => { if (mounted) { unlisten = fn; } else { fn(); } })
      .catch(console.error);
    return () => { mounted = false; unlisten?.(); };
  }, [onRefresh]);

  return lastWatcherEvent;
}
