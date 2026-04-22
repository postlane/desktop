// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Button } from '../components/catalyst/button';
import { Dialog, DialogActions, DialogBody, DialogDescription, DialogTitle } from '../components/catalyst/dialog';
import PostCard from './PostCard';
import type { DraftPost, MetaChangedPayload } from '../types';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

interface Props {
  postWizardNudge: boolean;
  onNudgeDismissed: () => void;
}

interface RepoGroup {
  repoId: string;
  repoName: string;
  posts: DraftPost[];
}

function groupAndSort(posts: DraftPost[]): RepoGroup[] {
  const map = new Map<string, RepoGroup>();
  for (const post of posts) {
    if (!map.has(post.repo_id)) {
      map.set(post.repo_id, { repoId: post.repo_id, repoName: post.repo_name, posts: [] });
    }
    map.get(post.repo_id)?.posts.push(post);
  }
  for (const group of map.values()) {
    group.posts.sort((a, b) => {
      if (a.status === 'failed' && b.status !== 'failed') return -1;
      if (b.status === 'failed' && a.status !== 'failed') return 1;
      return (b.created_at ?? '').localeCompare(a.created_at ?? '');
    });
  }
  return [...map.values()];
}

function WizardNudge({ onDismiss }: { onDismiss: () => void }) {
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'fallback'>('idle');

  async function handleCopy() {
    try { await writeText('/draft-post'); setCopyState('copied'); setTimeout(() => setCopyState('idle'), 2000); }
    catch { setCopyState('fallback'); }
  }

  return (
    <div className="flex h-full items-center justify-center p-8">
      <div className="max-w-sm text-center">
        <p className="mb-4 font-medium text-zinc-900 dark:text-zinc-100">You're set up.</p>
        <p className="mb-6 text-sm text-zinc-600 dark:text-zinc-400">Open your IDE in a registered repo and run:</p>
        <div className="mb-4 flex items-center justify-center gap-3 rounded-lg bg-zinc-100 px-4 py-3 dark:bg-zinc-800">
          <code className="font-mono text-sm text-zinc-900 dark:text-zinc-100">/draft-post</code>
          <Button plain onClick={handleCopy} aria-label="Copy /draft-post command">{copyState === 'copied' ? '✓ Copied' : '📋 Copy'}</Button>
        </div>
        {copyState === 'fallback' && <p className="mb-4 text-xs text-zinc-500">Press Ctrl+C to copy</p>}
        <p className="text-sm text-zinc-500">Your first draft will appear here when it's ready.</p>
        <Button plain onClick={onDismiss} className="mt-6 text-xs text-zinc-400">Dismiss</Button>
      </div>
    </div>
  );
}

function useAllReposDrafts() {
  const [posts, setPosts] = useState<DraftPost[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try { const result = await invoke<DraftPost[]>('get_all_drafts'); setPosts(result); }
    catch (e) { console.error('get_all_drafts failed:', e); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;
    listen<MetaChangedPayload>('meta-changed', () => refresh())
      .then((fn) => { if (mounted) { unlisten = fn; } else { fn(); } })
      .catch(console.error);
    return () => { mounted = false; unlisten?.(); };
  }, [refresh]);

  return { posts, loading, refresh };
}

interface ApproveAllDialogProps {
  open: boolean;
  readyCount: number;
  running: boolean;
  results: Map<string, 'ok' | 'error'>;
  onClose: () => void;
  onConfirm: () => void;
}

function ApproveAllDialog({ open, readyCount, running, results, onClose, onConfirm }: ApproveAllDialogProps) {
  return (
    <Dialog open={open} onClose={onClose}>
      <DialogTitle>Approve all ready posts</DialogTitle>
      <DialogDescription>Send {readyCount} post{readyCount !== 1 ? 's' : ''} to your scheduler?</DialogDescription>
      <DialogBody>
        {results.size > 0 && (
          <ul className="space-y-1">
            {[...results.entries()].map(([folder, result]) => (
              <li key={folder} className="flex items-center gap-2 text-sm">
                {result === 'ok' ? <span className="text-green-600">✓</span> : <span className="text-red-600">✗</span>}
                {folder}
              </li>
            ))}
          </ul>
        )}
      </DialogBody>
      <DialogActions>
        <Button plain onClick={onClose}>Cancel</Button>
        <Button color="green" onClick={onConfirm} disabled={running}>{running ? 'Sending…' : 'Confirm'}</Button>
      </DialogActions>
    </Dialog>
  );
}

export default function AllReposDraftsView({ postWizardNudge, onNudgeDismissed }: Props) {
  const { posts, loading, refresh } = useAllReposDrafts();
  const [approveAllOpen, setApproveAllOpen] = useState(false);
  const [approveAllResults, setApproveAllResults] = useState<Map<string, 'ok' | 'error'>>(new Map());
  const [approveAllRunning, setApproveAllRunning] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && posts.filter((p) => p.status === 'ready').length >= 2) { e.preventDefault(); setApproveAllOpen(true); }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [posts]);

  async function handleApproveAll() {
    setApproveAllRunning(true);
    for (const post of posts.filter((p) => p.status === 'ready')) {
      try {
        await invoke('approve_post', { repoPath: post.repo_path, postFolder: post.post_folder });
        setApproveAllResults((prev) => new Map(prev).set(post.post_folder, 'ok'));
      } catch { setApproveAllResults((prev) => new Map(prev).set(post.post_folder, 'error')); }
    }
    setApproveAllRunning(false);
    setApproveAllOpen(false);
    setApproveAllResults(new Map());
    await refresh();
  }

  const readyCount = posts.filter((p) => p.status === 'ready').length;
  const groups = groupAndSort(posts);

  if (postWizardNudge) return <WizardNudge onDismiss={onNudgeDismissed} />;
  if (loading) return <div className="flex h-full items-center justify-center"><p className="text-sm text-zinc-400">Loading…</p></div>;
  if (posts.length === 0) return (
    <div className="flex h-full items-center justify-center p-8">
      <p className="text-center text-sm text-zinc-500">No drafts waiting.<br />Invoke <code className="font-mono">/draft-post</code> in your IDE to create one.</p>
    </div>
  );

  return (
    <>
      <div className="flex items-center justify-between border-b border-zinc-200 px-6 py-4 dark:border-zinc-700">
        <h1 className="text-base font-semibold text-zinc-900 dark:text-zinc-100">All repos — Drafts</h1>
        {readyCount >= 2 && <Button color="green" onClick={() => setApproveAllOpen(true)}>Approve all ready ({readyCount})</Button>}
      </div>
      <div className="space-y-6 p-6">
        {groups.map((group) => (
          <section key={group.repoId}>
            <h2 className="mb-3 text-xs font-semibold uppercase tracking-wider text-zinc-500 dark:text-zinc-400">{group.repoName}</h2>
            <div className="space-y-3">
              {group.posts.map((post) => (
                <PostCard key={`${post.repo_id}-${post.post_folder}`} post={post} isFocused={posts.indexOf(post) === 0} onApproved={refresh} onDismissed={refresh} />
              ))}
            </div>
          </section>
        ))}
      </div>
      <ApproveAllDialog open={approveAllOpen} readyCount={readyCount} running={approveAllRunning} results={approveAllResults} onClose={() => setApproveAllOpen(false)} onConfirm={handleApproveAll} />
    </>
  );
}
