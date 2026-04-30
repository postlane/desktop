// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { confirm } from '@tauri-apps/plugin-dialog';
import type { DraftPost } from '../types';

export function usePostCardRedraft(post: DraftPost) {
  const [redraftInstruction, setRedraftInstruction] = useState('');
  const [redraftQueued, setRedraftQueued] = useState(false);
  const [redraftError, setRedraftError] = useState<string | null>(null);

  const handleQueueRedraft = useCallback(async () => {
    const ok = await confirm(`Queue for redraft with instruction: "${redraftInstruction.trim()}"?`, { title: 'Confirm redraft', kind: 'info' });
    if (!ok) return;
    try { await invoke('queue_redraft', { repoPath: post.repo_path, postFolder: post.post_folder, instruction: redraftInstruction.trim() }); setRedraftQueued(true); setRedraftError(null); }
    catch (e) { setRedraftError(e instanceof Error ? e.message : String(e)); }
  }, [post, redraftInstruction]);

  const handleCancelRedraft = useCallback(async () => {
    try { await invoke('cancel_redraft', { repoPath: post.repo_path }); setRedraftQueued(false); setRedraftInstruction(''); setRedraftError(null); }
    catch (e) { console.error('cancel_redraft failed:', e); }
  }, [post]);

  const handleInstructionChange = useCallback((v: string) => { setRedraftInstruction(v); setRedraftQueued(false); setRedraftError(null); }, []);

  return { redraftInstruction, redraftQueued, redraftError, handleQueueRedraft, handleCancelRedraft, handleInstructionChange };
}
