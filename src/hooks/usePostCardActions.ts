// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback } from 'react';
import { invoke } from '../ipc/invoke';
import { confirm } from '@tauri-apps/plugin-dialog';
import type { DraftPost } from '../types';

// Mirrors Rust's ApproveError (checklist 24.4.11) as delivered by Tauri's
// invoke() rejection — a plain tagged object, not a wrapped Error.
export interface ApproveBlockedInfo {
  status: string;
  isOwner: boolean;
  daysRemaining: number | null;
}

interface RawBlockedError {
  kind: 'blocked';
  status: string;
  is_owner: boolean;
  days_remaining: number | null;
}

function isBlockedError(e: unknown): e is RawBlockedError {
  return typeof e === 'object' && e !== null && (e as { kind?: unknown }).kind === 'blocked';
}

function approveErrorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === 'object' && e !== null && typeof (e as { message?: unknown }).message === 'string') {
    return (e as { message: string }).message;
  }
  return String(e);
}

export function usePostCardActions(post: DraftPost, onApproved: () => void, onDismissed: () => void) {
  const [approving, setApproving] = useState(false);
  const [approveError, setApproveError] = useState<string | null>(null);
  const [approveBlockedInfo, setApproveBlockedInfo] = useState<ApproveBlockedInfo | null>(null);
  const [approveSuccessPlatforms, setApproveSuccessPlatforms] = useState<string[] | null>(null);
  const [fallbackNotice, setFallbackNotice] = useState<string | null>(null);
  const [retrying, setRetrying] = useState(false);
  const [retryError, setRetryError] = useState<string | null>(null);
  const [dismissError, setDismissError] = useState<string | null>(null);

  const approve = useCallback(async () => {
    setApproving(true); setApproveError(null); setApproveBlockedInfo(null);
    try {
      // approve_post returns Result<(), ApproveError> which serialises to null on success.
      // Treat a successful (non-throwing) call as approval complete.
      await invoke('approve_post', {
        repoPath: post.repo_path,
        postFolder: post.post_folder,
        platform: post.platform ?? '',
      });
      setApproveSuccessPlatforms(post.platforms ?? []);
    }
    catch (e) {
      if (isBlockedError(e)) {
        setApproveBlockedInfo({ status: e.status, isOwner: e.is_owner, daysRemaining: e.days_remaining });
      } else {
        setApproveError(approveErrorMessage(e));
      }
    }
    finally { setApproving(false); }
  }, [post]);

  const onSuccessDismissed = useCallback(() => { setApproveSuccessPlatforms(null); onApproved(); }, [onApproved]);

  const dismissFallbackNotice = useCallback(() => { setFallbackNotice(null); onApproved(); }, [onApproved]);

  const dismiss = useCallback(async () => {
    const yes = await confirm('Delete this post? This cannot be undone.', { title: 'Delete post', kind: 'warning' });
    if (!yes) return;
    setDismissError(null);
    try { await invoke('delete_post', { repoPath: post.repo_path, postFolder: post.post_folder }); onDismissed(); }
    catch (e) { setDismissError(e instanceof Error ? e.message : String(e)); }
  }, [post, onDismissed]);

  const retry = useCallback(async () => {
    setRetrying(true); setRetryError(null);
    try { await invoke('retry_post', { repoPath: post.repo_path, postFolder: post.post_folder }); onApproved(); }
    catch (e) { setRetryError(e instanceof Error ? e.message : String(e)); }
    finally { setRetrying(false); }
  }, [post, onApproved]);

  return { approving, approveError, approveBlockedInfo, approveSuccessPlatforms, onSuccessDismissed, fallbackNotice, dismissFallbackNotice, retrying, retryError, dismissError, approve, dismiss, retry };
}
