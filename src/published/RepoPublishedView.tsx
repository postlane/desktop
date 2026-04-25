// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/catalyst/button';
import { useTimezone, formatTimestamp } from '../TimezoneContext';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../components/catalyst/table';
import type { PublishedPost, PostAnalytics } from '../types';

const PAGE_SIZE = 100;

interface Props {
  repoId: string;
}

function ScheduledRow({ post, onCancelled, tz }: { post: PublishedPost; onCancelled: () => void; tz: string }) {
  const [cancelling, setCancelling] = useState(false);
  const [cancelError, setCancelError] = useState<string | null>(null);

  const firstPlatform = post.platforms[0] ?? 'x';
  const postId = post.scheduler_ids?.[firstPlatform] ?? '';

  async function handleCancel() {
    setCancelling(true);
    setCancelError(null);
    try {
      await invoke('cancel_post_command', { repoPath: post.repo_path, postFolder: post.post_folder, postId, platform: firstPlatform });
      onCancelled();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setCancelError(msg.toLowerCase().includes('not supported') ? 'Cancel via dashboard' : msg);
    } finally { setCancelling(false); }
  }

  return (
    <TableRow>
      <TableCell className="font-mono text-xs">{post.post_folder}</TableCell>
      <TableCell>{post.platforms.join(', ')}</TableCell>
      <TableCell>{formatTimestamp(post.schedule, tz)}</TableCell>
      <TableCell>
        {cancelError ? <span className="text-xs text-zinc-500">{cancelError}</span> : <Button outline onClick={handleCancel} disabled={cancelling}>{cancelling ? 'Cancelling…' : 'Cancel'}</Button>}
      </TableCell>
    </TableRow>
  );
}

function usePostAnalytics(repoId: string, postFolder: string) {
  const [analytics, setAnalytics] = useState<PostAnalytics | null>(null);
  useEffect(() => {
    invoke<PostAnalytics>('get_post_analytics', { repoId, postFolder })
      .then(setAnalytics)
      .catch(() => setAnalytics(null));
  }, [repoId, postFolder]);
  return analytics;
}

function AnalyticsCell({ repoId, postFolder }: { repoId: string; postFolder: string }) {
  const analytics = usePostAnalytics(repoId, postFolder);
  if (!analytics) return <span className="text-zinc-400">—</span>;
  if (analytics.unique_sessions === 0) return <span className="text-zinc-400">0 sessions</span>;
  return <span>{analytics.unique_sessions} sessions{analytics.top_referrer ? ` · ${analytics.top_referrer}` : ''}</span>;
}

function SentRow({ post, tz }: { post: PublishedPost; tz: string }) {
  const sentPlatforms = post.platform_results
    ? Object.entries(post.platform_results).filter(([, v]) => v === 'sent').map(([k]) => k)
    : post.platforms;
  const viewLinks = sentPlatforms
    .map((platform) => ({ platform, url: post.platform_urls?.[platform] ?? null }))
    .filter((l): l is { platform: string; url: string } => l.url !== null);

  async function handleOpenLink(url: string) {
    try { await invoke('plugin:opener|open_url', { url }); }
    catch (e) { console.error('Failed to open URL:', e); }
  }

  return (
    <TableRow>
      <TableCell className="font-mono text-xs">{post.post_folder}</TableCell>
      <TableCell className="text-xs text-zinc-500">{formatTimestamp(post.sent_at, tz)}</TableCell>
      <TableCell className="text-xs">{sentPlatforms.join(', ')}</TableCell>
      <TableCell className="text-xs">{post.llm_model ?? '—'}</TableCell>
      <TableCell className="text-xs"><AnalyticsCell repoId={post.repo_id} postFolder={post.post_folder} /></TableCell>
      <TableCell className="text-xs">
        {viewLinks.length > 0 ? viewLinks.map((l) => (
          <button key={l.platform} onClick={() => handleOpenLink(l.url)} aria-label={`View ${l.platform} post`} className="mr-2 text-blue-600 underline hover:text-blue-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 dark:text-blue-400 dark:hover:text-blue-200">{l.platform} ↗</button>
        )) : '—'}
      </TableCell>
    </TableRow>
  );
}

function useRepoPublished(repoId: string) {
  const [posts, setPosts] = useState<PublishedPost[]>([]);
  const [page, setPage] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);

  const loadPage = useCallback(async (pageIndex: number, append: boolean) => {
    try {
      const result = await invoke<PublishedPost[]>('get_repo_published', { repoId, offset: pageIndex * PAGE_SIZE, limit: PAGE_SIZE + 1 });
      const hasMoreResults = result.length > PAGE_SIZE;
      const slice = hasMoreResults ? result.slice(0, PAGE_SIZE) : result;
      setPosts((prev) => (append ? [...prev, ...slice] : slice));
      setHasMore(hasMoreResults);
    } catch (e) { console.error('get_repo_published failed:', e); }
    finally { setLoading(false); }
  }, [repoId]);

  useEffect(() => { setPage(0); setPosts([]); setLoading(true); loadPage(0, false); }, [repoId, loadPage]);

  const loadMore = () => { const next = page + 1; setPage(next); loadPage(next, true); };
  const refresh = useCallback(() => loadPage(0, false), [loadPage]);

  return { posts, hasMore, loading, loadMore, refresh };
}

export default function RepoPublishedView({ repoId }: Props) {
  const tz = useTimezone();
  const { posts, hasMore, loading, loadMore, refresh } = useRepoPublished(repoId);

  if (loading) return <div className="flex h-full items-center justify-center"><p className="text-sm text-zinc-400">Loading…</p></div>;

  const queued = posts.filter((p) => p.status === 'queued');
  const sent = posts.filter((p) => p.status === 'sent');

  if (posts.length === 0) return (
    <div className="flex h-full items-center justify-center p-8">
      <p className="text-center text-sm text-zinc-500">No posts sent yet. Draft your first post with <code className="font-mono">/draft-post</code> in your IDE.</p>
    </div>
  );

  return (
    <div className="p-6 space-y-8">
      {queued.length > 0 && (
        <section>
          <h2 role="heading" className="mb-3 text-sm font-semibold text-zinc-700 dark:text-zinc-300">Scheduled</h2>
          <Table>
            <TableHead><TableRow><TableHeader>Post</TableHeader><TableHeader>Platforms</TableHeader><TableHeader>Scheduled for</TableHeader><TableHeader></TableHeader></TableRow></TableHead>
            <TableBody>{queued.map((post) => <ScheduledRow key={post.post_folder} post={post} tz={tz} onCancelled={refresh} />)}</TableBody>
          </Table>
        </section>
      )}
      {sent.length > 0 && (
        <section>
          <Table>
            <TableHead><TableRow><TableHeader>Slug</TableHeader><TableHeader>Sent</TableHeader><TableHeader>Platforms</TableHeader><TableHeader>Model</TableHeader><TableHeader>Engagement</TableHeader><TableHeader>Links</TableHeader></TableRow></TableHead>
            <TableBody>{sent.map((post) => <SentRow key={post.post_folder} post={post} tz={tz} />)}</TableBody>
          </Table>
          {hasMore && <div className="mt-4 text-center"><Button outline onClick={loadMore}>Load more</Button></div>}
        </section>
      )}
    </div>
  );
}
