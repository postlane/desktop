// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { DraftPost, Platform } from '../types';

export function usePostCardContent(post: DraftPost, activeTab: Platform) {
  const [postContent, setPostContent] = useState<string>('');
  const [contentLoadError, setContentLoadError] = useState<string | null>(null);
  const [attributionEnabled, setAttributionEnabled] = useState(true);

  useEffect(() => {
    invoke<string>('get_post_content', { repoPath: post.repo_path, postFolder: post.post_folder, platform: activeTab })
      .then((c) => { setPostContent(typeof c === 'string' ? c : ''); setContentLoadError(null); })
      .catch((e) => { setPostContent(''); setContentLoadError('Failed to load post content.'); console.error('get_post_content failed:', e); });
  }, [activeTab, post.repo_path, post.post_folder]);

  useEffect(() => { invoke<boolean>('get_attribution').then((v) => setAttributionEnabled(v)).catch(() => {}); }, []);

  return { postContent, setPostContent, contentLoadError, attributionEnabled };
}
