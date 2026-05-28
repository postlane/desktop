// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { invoke } from '../ipc/invoke';
import { listen } from '@tauri-apps/api/event';
import PostCard from './PostCard';
import type { DraftPost, MetaChangedPayload } from '../types';
import { isDraftPost } from '../ipc-guards';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { useConnectedPlatforms } from '../hooks/useConnectedPlatforms';

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
    try { await writeText('npx @postlane/cli draft-post'); setCopyState('copied'); setTimeout(() => setCopyState('idle'), 2000); }
    catch { setCopyState('fallback'); }
  }

  return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%', padding: '2rem' }}>
      <div className="has-text-centered" style={{ maxWidth: '24rem' }}>
        <p className="has-text-weight-medium mb-4">You're set up.</p>
        <p className="is-size-7 has-text-grey mb-5">Open a terminal in a registered repo and run:</p>
        <div className="is-flex is-align-items-center is-justify-content-center has-background-grey-lighter mb-4" style={{ borderRadius: '0.5rem', padding: '0.75rem 1rem', gap: '0.75rem' }}>
          <code className="is-size-7">npx @postlane/cli draft-post</code>
          <button className="button is-ghost is-small" onClick={handleCopy} aria-label="Copy draft-post command">{copyState === 'copied' ? '✓ Copied' : '📋 Copy'}</button>
        </div>
        {copyState === 'fallback' && <p className="is-size-7 has-text-grey mb-4">Press Ctrl+C to copy</p>}
        <p className="is-size-7 has-text-grey">Your first draft will appear here when it's ready.</p>
        <button className="button is-ghost is-small has-text-grey-light mt-5" onClick={onDismiss}>Dismiss</button>
      </div>
    </div>
  );
}

function useAllReposDrafts() {
  const [posts, setPosts] = useState<DraftPost[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<unknown[]>('get_all_drafts');
      setPosts(result.filter(isDraftPost));
      setError(null);
    }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
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

  return { posts, loading, error, refresh };
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
  if (!open) return null;
  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onClose} />
      <div className="modal-card" role="dialog" aria-modal="true">
        <header className="modal-card-head">
          <p className="modal-card-title">Approve all ready posts</p>
          <button className="delete" onClick={onClose} aria-label="Close" />
        </header>
        <section className="modal-card-body">
          <p className="is-size-7 has-text-grey mb-3">Send {readyCount} post{readyCount !== 1 ? 's' : ''} to your scheduler?</p>
          {results.size > 0 && (
            <ul>
              {[...results.entries()].map(([folder, result]) => (
                <li key={folder} className="is-flex is-align-items-center is-size-7" style={{ gap: '0.5rem' }}>
                  {result === 'ok' ? <span className="has-text-success">✓</span> : <span className="has-text-danger">✗</span>}
                  {folder}
                </li>
              ))}
            </ul>
          )}
        </section>
        <footer className="modal-card-foot is-justify-content-flex-end" style={{ gap: '0.5rem' }}>
          <button className="button is-ghost" onClick={onClose}>Cancel</button>
          <button className="button is-success" onClick={onConfirm} disabled={running}>{running ? 'Sending…' : 'Confirm'}</button>
        </footer>
      </div>
    </div>
  );
}

function DraftsError({ message }: { message: string }) {
  return (
    <div role="alert" className="notification is-danger is-light mx-5 mt-5 is-size-7">
      Failed to load drafts: {message}
    </div>
  );
}

function EmptyDraftsState() {
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'error'>('idle');
  async function handleCopy() {
    try { await writeText('npx @postlane/cli draft-post'); setCopyState('copied'); setTimeout(() => setCopyState('idle'), 2000); }
    catch { setCopyState('error'); setTimeout(() => setCopyState('idle'), 2000); }
  }
  return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%', padding: '2rem' }}>
      <div className="has-text-centered">
        <p className="is-size-7 has-text-grey mb-3">No drafts waiting.</p>
        <p className="is-size-7 has-text-grey mb-4">Run this command in a terminal inside your repo:</p>
        <div className="is-flex is-align-items-center is-justify-content-center has-background-grey-lighter" style={{ borderRadius: '0.5rem', padding: '0.75rem 1rem', gap: '0.75rem' }}>
          <code className="is-size-7">npx @postlane/cli draft-post</code>
          <button className="button is-ghost is-small" onClick={handleCopy} aria-label="Copy draft-post command">
            {copyState === 'copied' ? '✓ Copied' : copyState === 'error' ? 'Failed to copy' : '📋 Copy'}
          </button>
        </div>
      </div>
    </div>
  );
}

function useHasUnsplashKey() {
  const [hasUnsplashKey, setHasUnsplashKey] = useState(false);
  useEffect(() => {
    invoke<boolean>('has_unsplash_key').then(setHasUnsplashKey).catch(() => setHasUnsplashKey(false));
  }, []);
  return hasUnsplashKey;
}

export default function AllReposDraftsView({ postWizardNudge, onNudgeDismissed }: Props) {
  const { posts, loading, error, refresh } = useAllReposDrafts();
  const hasUnsplashKey = useHasUnsplashKey();
  const connectedPlatformsByRepo = useConnectedPlatforms(useMemo(() => [...new Set(posts.map((p) => p.repo_id))], [posts]));
  const [approveAllOpen, setApproveAllOpen] = useState(false);
  const [approveAllResults, setApproveAllResults] = useState<Map<string, 'ok' | 'error'>>(new Map());
  const [approveAllRunning, setApproveAllRunning] = useState(false);
  const postsRef = useRef(posts);
  postsRef.current = posts;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && postsRef.current.filter((p) => p.status === 'ready').length >= 2) { e.preventDefault(); setApproveAllOpen(true); }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, []);

  async function handleApproveAll() {
    setApproveAllRunning(true);
    for (const post of posts.filter((p) => p.status === 'ready')) {
      try {
        await invoke('approve_post', { repoPath: post.repo_path, postFolder: post.post_folder, platform: post.platform ?? '' });
        setApproveAllResults((prev) => new Map(prev).set(post.post_folder, 'ok'));
      } catch { setApproveAllResults((prev) => new Map(prev).set(post.post_folder, 'error')); }
    }
    setApproveAllRunning(false);
    setApproveAllOpen(false);
    setApproveAllResults(new Map());
    await refresh();
  }

  const readyCount = posts.filter((p) => p.status === 'ready').length;

  if (postWizardNudge) return <WizardNudge onDismiss={onNudgeDismissed} />;
  if (loading) return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%' }}>
      <p className="is-size-7 has-text-grey">Loading…</p>
    </div>
  );
  if (error) return <DraftsError message={error} />;
  if (posts.length === 0) return <EmptyDraftsState />;

  return (
    <>
      <div className="is-flex is-align-items-center is-justify-content-space-between px-5 py-4" style={{ borderBottom: '1px solid var(--bulma-border-weak)' }}>
        <h1 className="has-text-weight-semibold">All repos — Drafts</h1>
        {readyCount >= 2 && <button className="button is-success is-small" onClick={() => setApproveAllOpen(true)}>Approve all ready ({readyCount})</button>}
      </div>
      <div className="p-5" style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem' }}>
        {groupAndSort(posts).map((group) => (
          <section key={group.repoId}>
            <h2 className="has-text-grey is-size-7 has-text-weight-semibold mb-3" style={{ textTransform: 'uppercase', letterSpacing: '0.05em' }}>{group.repoName}</h2>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
              {group.posts.map((post) => (
                <PostCard key={`${post.repo_id}-${post.post_folder}`} post={post} isFocused={posts.indexOf(post) === 0} connectedPlatforms={connectedPlatformsByRepo[group.repoId]} hasUnsplashKey={hasUnsplashKey} onApproved={refresh} onDismissed={refresh} />
              ))}
            </div>
          </section>
        ))}
      </div>
      <ApproveAllDialog open={approveAllOpen} readyCount={readyCount} running={approveAllRunning} results={approveAllResults} onClose={() => setApproveAllOpen(false)} onConfirm={handleApproveAll} />
    </>
  );
}
