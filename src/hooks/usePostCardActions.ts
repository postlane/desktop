// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback } from 'react';
import { invoke } from '../ipc/invoke';
import { confirm } from '@tauri-apps/plugin-dialog';
import type { DraftPost, SendResult } from '../types';

export function usePostCardActions(post: DraftPost, onApproved: () => void, onDismissed: () => void) {
  const [approving, setApproving] = useState(false);
  const [approveError, setApproveError] = useState<string | null>(null);
  const [approveSuccessPlatforms, setApproveSuccessPlatforms] = useState<string[] | null>(null);
  const [fallbackNotice, setFallbackNotice] = useState<string | null>(null);
  const [retrying, setRetrying] = useState(false);
  const [retryError, setRetryError] = useState<string | null>(null);

  const approve = useCallback(async () => {
    setApproving(true); setApproveError(null);
    try {
      const result = await invoke<SendResult>('approve_post', { repoPath: post.repo_path, postFolder: post.post_folder });
      if (result.fallback_provider) {
        setFallbackNotice(result.fallback_provider);
      } else {
        const sentPlatforms = Object.entries(result.platform_results ?? {})
          .filter(([, v]) => v === 'sent' || v === 'success')
          .map(([k]) => k);
        setApproveSuccessPlatforms(sentPlatforms);
      }
    }
    catch (e) { setApproveError(e instanceof Error ? e.message : String(e)); }
    finally { setApproving(false); }
  }, [post]);

  const onSuccessDismissed = useCallback(() => { setApproveSuccessPlatforms(null); onApproved(); }, [onApproved]);

  const dismissFallbackNotice = useCallback(() => { setFallbackNotice(null); onApproved(); }, [onApproved]);

  const dismiss = useCallback(async () => {
    const yes = await confirm('Delete this post? This cannot be undone.', { title: 'Delete post', kind: 'warning' });
    if (!yes) return;
    try { await invoke('delete_post', { repoPath: post.repo_path, postFolder: post.post_folder }); onDismissed(); }
    catch (e) { console.error('delete_post failed:', e); }
  }, [post, onDismissed]);

  const retry = useCallback(async () => {
    setRetrying(true); setRetryError(null);
    try { await invoke('retry_post', { repoPath: post.repo_path, postFolder: post.post_folder }); onApproved(); }
    catch (e) { setRetryError(e instanceof Error ? e.message : String(e)); }
    finally { setRetrying(false); }
  }, [post, onApproved]);

  return { approving, approveError, approveSuccessPlatforms, onSuccessDismissed, fallbackNotice, dismissFallbackNotice, retrying, retryError, approve, dismiss, retry };
}
