// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Platform } from '../types';

export function useMastodonCharLimit(activeTab: Platform) {
  const [charLimit, setCharLimit] = useState<number | undefined>(undefined);
  useEffect(() => {
    if (activeTab !== 'mastodon') { setCharLimit(undefined); return; }
    invoke<string | null>('get_mastodon_connected_instance')
      .then((instance) => {
        if (!instance) return;
        return invoke<number>('get_mastodon_char_limit', { instance });
      })
      .then((limit) => { if (limit !== undefined) setCharLimit(limit); })
      .catch(() => {});
  }, [activeTab]);
  return charLimit;
}
