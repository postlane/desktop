// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTimezone, formatTimestamp } from '../TimezoneContext';
import { Button } from '../components/catalyst/button';
import { Badge } from '../components/catalyst/badge';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../components/catalyst/table';
import type { PublishedPost, ModelStats, PostAnalytics } from '../types';

const PAGE_SIZE = 100;

interface Props {
  onNavigateToRepo: (_repoId: string) => void;
}

function ModelBar({ stats }: { stats: ModelStats[] }) {
  const maxRate = Math.max(...stats.map((s) => s.edit_rate), 0.01);

  return (
    <section className="mb-8 rounded-xl border border-zinc-200 bg-white p-5 dark:border-zinc-700 dark:bg-zinc-900">
      <div className="mb-3 flex items-center gap-2">
        <h2 className="text-sm font-semibold text-zinc-700 dark:text-zinc-300">Edit rate by model</h2>
        <span title="Edit rate = how often you changed the draft before approving. Does not account for dismissed posts." className="cursor-help text-xs text-zinc-400">ⓘ</span>
      </div>
      <div className="space-y-3">
        {stats.map((s) => {
          const pct = Math.round(s.edit_rate * 100);
          const barWidth = Math.round((s.edit_rate / maxRate) * 100);
          return (
            <div key={s.model}>
              <div className="mb-1 flex items-center justify-between text-xs">
                <span className={s.limited_data ? 'text-zinc-400' : 'text-zinc-700 dark:text-zinc-300'}>
                  {s.model}
                  {s.limited_data && <span className="ml-2 text-zinc-400">Limited data ({s.total_posts} posts)</span>}
                </span>
                <span className={s.limited_data ? 'text-zinc-400' : 'text-zinc-600 dark:text-zinc-400'}>{pct}% edited ({s.total_posts} posts)</span>
              </div>
              <div className="h-2 w-full rounded-full bg-zinc-100 dark:bg-zinc-800">
                <div className={`h-2 rounded-full ${s.limited_data ? 'bg-zinc-300 dark:bg-zinc-600' : 'bg-zinc-700 dark:bg-zinc-300'}`} style={{ width: `${barWidth}%` }} />
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}

function useAllPublished() {
  const [posts, setPosts] = useState<PublishedPost[]>([]);
  const [stats, setStats] = useState<ModelStats[]>([]);
  const [page, setPage] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);

  const loadPage = useCallback(async (pageIndex: number, append: boolean) => {
    try {
      const [result, modelStats] = await Promise.all([
        invoke<PublishedPost[]>('get_all_published', { offset: pageIndex * PAGE_SIZE, limit: PAGE_SIZE + 1 }),
        pageIndex === 0 ? invoke<ModelStats[]>('get_model_stats') : Promise.resolve(null),
      ]);
      const hasMoreResults = result.length > PAGE_SIZE;
      const slice = hasMoreResults ? result.slice(0, PAGE_SIZE) : result;
      setPosts((prev) => (append ? [...prev, ...slice] : slice));
      setHasMore(hasMoreResults);
      if (modelStats) setStats(modelStats);
    } catch (e) { console.error('get_all_published failed:', e); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => { setPage(0); setPosts([]); setLoading(true); loadPage(0, false); }, [loadPage]);

  const loadNextPage = useCallback(() => { const next = page + 1; setPage(next); loadPage(next, true); }, [page, loadPage]);

  return { posts, stats, hasMore, loading, loadNextPage };
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
  const a = usePostAnalytics(repoId, postFolder);
  if (!a) return <span className="text-zinc-400">—</span>;
  if (a.unique_sessions === 0) return <span className="text-zinc-400">0</span>;
  return <span title={a.top_referrer ?? undefined}>{a.unique_sessions}</span>;
}

function PublishedPostRow({ post, onOpenLink, onNavigateToRepo }: { post: PublishedPost; onOpenLink: (_url: string) => void; onNavigateToRepo: (_repoId: string) => void }) {
  const tz = useTimezone();
  const sentPlatforms = post.platform_results
    ? Object.entries(post.platform_results).filter(([, v]) => v === 'sent').map(([k]) => k)
    : post.platforms;
  const viewLinks = sentPlatforms
    .map((platform) => ({ platform, url: post.platform_urls?.[platform] ?? null }))
    .filter((l): l is { platform: string; url: string } => l.url !== null);

  return (
    <TableRow>
      <TableCell><button onClick={() => onNavigateToRepo(post.repo_id)} className="focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"><Badge color="zinc">{post.repo_name}</Badge></button></TableCell>
      <TableCell className="font-mono text-xs">{post.post_folder}</TableCell>
      <TableCell className="text-xs text-zinc-500">{formatTimestamp(post.sent_at, tz)}</TableCell>
      <TableCell className="text-xs">{sentPlatforms.join(', ')}</TableCell>
      <TableCell className="text-xs capitalize">{post.provider ?? '—'}</TableCell>
      <TableCell className="text-xs">{post.llm_model ?? '—'}</TableCell>
      <TableCell className="text-xs"><AnalyticsCell repoId={post.repo_id} postFolder={post.post_folder} /></TableCell>
      <TableCell className="text-xs">
        {viewLinks.length > 0 ? viewLinks.map((l) => (
          <button key={l.platform} onClick={() => onOpenLink(l.url)} aria-label={`View ${l.platform} post`} className="mr-2 text-blue-600 underline hover:text-blue-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 dark:text-blue-400 dark:hover:text-blue-200">{l.platform} ↗</button>
        )) : '—'}
      </TableCell>
    </TableRow>
  );
}

export default function AllReposPublishedView({ onNavigateToRepo }: Props) {
  const [exportStatus, setExportStatus] = useState<string | null>(null);
  const { posts, stats, hasMore, loading, loadNextPage } = useAllPublished();

  async function handleExport() {
    setExportStatus(null);
    try { const path = await invoke<string>('export_history_csv'); setExportStatus(`Saved to ${path}`); }
    catch (e) { setExportStatus(e instanceof Error ? e.message : 'Export failed'); }
  }

  async function handleOpenLink(url: string) {
    try { await invoke('plugin:opener|open_url', { url }); }
    catch (e) { console.error('Failed to open URL:', e); }
  }

  if (loading) return <div className="flex h-full items-center justify-center"><p className="text-sm text-zinc-400">Loading…</p></div>;

  const sentPosts = posts.filter((p) => p.status === 'sent');
  const showModelBar = sentPosts.length >= 10 && stats.length > 0;

  if (posts.length === 0) return (
    <div className="flex h-full items-center justify-center p-8">
      <p className="text-center text-sm text-zinc-500">No posts published yet. Draft your first post with <code className="font-mono">/draft-post</code> in your IDE.</p>
    </div>
  );

  return (
    <div className="p-6">
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-base font-semibold text-zinc-900 dark:text-zinc-100">All repos — Published</h1>
        <div className="flex items-center gap-3">
          {exportStatus && <span className="text-xs text-zinc-500">{exportStatus}</span>}
          <Button outline onClick={handleExport}>Export CSV</Button>
        </div>
      </div>
      {showModelBar && <ModelBar stats={stats} />}
      <Table>
        <TableHead>
          <TableRow>
            <TableHeader>Repo</TableHeader><TableHeader>Slug</TableHeader><TableHeader>Sent</TableHeader>
            <TableHeader>Platforms</TableHeader><TableHeader>Scheduler</TableHeader><TableHeader>Model</TableHeader>
            <TableHeader>Engagement</TableHeader><TableHeader>Links</TableHeader>
          </TableRow>
        </TableHead>
        <TableBody>
          {sentPosts.map((post) => <PublishedPostRow key={`${post.repo_id}-${post.post_folder}`} post={post} onOpenLink={handleOpenLink} onNavigateToRepo={onNavigateToRepo} />)}
        </TableBody>
      </Table>
      {hasMore && <div className="mt-4 text-center"><Button outline onClick={loadNextPage}>Load more</Button></div>}
    </div>
  );
}
