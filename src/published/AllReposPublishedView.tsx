// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTimezone, formatTimestamp } from '../TimezoneContext';
import type { PublishedPost, ModelStats, PostAnalytics } from '../types';
import { isPublishedPost } from '../ipc-guards';

const PAGE_SIZE = 100;

interface Props {
  onNavigateToRepo: (_repoId: string) => void;
}

function ModelBar({ stats }: { stats: ModelStats[] }) {
  const maxRate = Math.max(...stats.map((s) => s.edit_rate), 0.01);

  return (
    <section className="box mb-5">
      <div className="mb-3">
        <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
          <h2 className="has-text-weight-semibold is-size-7">Edit rate by model</h2>
          <span title="Edit rate = how often you changed the draft before approving. Does not account for dismissed posts." className="has-text-grey-light is-size-7" style={{ cursor: 'help' }}>ⓘ</span>
        </div>
        <p className="is-size-7 has-text-grey mt-1">How often posts needed editing before sending — lower is better.</p>
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
        {stats.map((s) => {
          const pct = Math.round(s.edit_rate * 100);
          const barWidth = Math.round((s.edit_rate / maxRate) * 100);
          return (
            <div key={s.model}>
              <div className="is-flex is-justify-content-space-between is-size-7 mb-1">
                <span className={s.limited_data ? 'has-text-grey-light' : 'has-text-grey-dark'}>
                  {s.model}
                  {s.limited_data && <span className="has-text-grey-light ml-2">Limited data ({s.total_posts} posts)</span>}
                </span>
                <span className={s.limited_data ? 'has-text-grey-light' : 'has-text-grey'}>{pct}% edited ({s.total_posts} posts)</span>
              </div>
              <progress className="progress is-small" value={barWidth} max={100} style={{ height: '0.5rem' }} />
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
    const [postsResult, statsResult] = await Promise.allSettled([
      invoke<unknown[]>('get_all_published', { offset: pageIndex * PAGE_SIZE, limit: PAGE_SIZE + 1 }),
      pageIndex === 0 ? invoke<ModelStats[]>('get_model_stats') : Promise.resolve(null),
    ]);
    if (postsResult.status === 'fulfilled') {
      const result = postsResult.value.filter(isPublishedPost);
      const hasMoreResults = result.length > PAGE_SIZE;
      const slice = hasMoreResults ? result.slice(0, PAGE_SIZE) : result;
      setPosts((prev) => (append ? [...prev, ...slice] : slice));
      setHasMore(hasMoreResults);
    } else {
      console.error('get_all_published failed:', postsResult.reason);
    }
    if (statsResult.status === 'fulfilled' && statsResult.value) {
      setStats(statsResult.value);
    } else if (statsResult.status === 'rejected') {
      console.error('get_model_stats failed:', statsResult.reason);
    }
    setLoading(false);
  }, []);

  useEffect(() => { setPage(0); setPosts([]); setLoading(true); loadPage(0, false); }, [loadPage]);

  const loadNextPage = useCallback(() => { const next = page + 1; setPage(next); loadPage(next, true); }, [page, loadPage]);

  return { posts, stats, hasMore, loading, loadNextPage };
}

function AnalyticsToggleCell({ repoId, postFolder, sentAt }: { repoId: string; postFolder: string; sentAt?: string | null }) {
  const [analytics, setAnalytics] = useState<PostAnalytics | null>(null);
  const [loading, setLoading] = useState(false);
  const [triggered, setTriggered] = useState(false);

  async function handleLoad() {
    setTriggered(true);
    setLoading(true);
    try {
      const data = await invoke<PostAnalytics>('get_post_analytics', { repoId, postFolder });
      setAnalytics(data);
    } catch { setAnalytics(null); }
    finally { setLoading(false); }
  }

  if (!triggered) return (
    <button aria-label="Load analytics" title="Click to load analytics" onClick={handleLoad} className="button is-ghost is-small has-text-grey-light">—</button>
  );
  if (loading) return <span className="has-text-grey-light is-size-7">…</span>;
  if (!analytics?.configured) return <span className="has-text-grey-light is-size-7">Set up Analytics — Settings → Analytics</span>;
  if (analytics.unique_sessions === 0) {
    const isRecent = sentAt != null && (Date.now() - new Date(sentAt).getTime()) < 7 * 24 * 60 * 60 * 1000;
    return <span className="has-text-grey-light is-size-7">{isRecent ? 'No sessions yet' : 'No Postlane-referred sessions in the last 30 days'}</span>;
  }
  return <span className="is-size-7">{analytics.unique_sessions} unique · {analytics.sessions} total{analytics.top_referrer ? ` · ${analytics.top_referrer}` : ''}</span>;
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
    <tr>
      <td><button onClick={() => onNavigateToRepo(post.repo_id)} className="button is-ghost is-small"><span className="tag is-light">{post.repo_name}</span></button></td>
      <td className="is-family-monospace is-size-7">{post.post_folder}</td>
      <td className="has-text-grey is-size-7">{formatTimestamp(post.sent_at, tz)}</td>
      <td className="is-size-7">{sentPlatforms.join(', ')}</td>
      <td className="is-size-7 is-capitalized">{post.provider ?? '—'}</td>
      <td className="is-size-7">{post.llm_model ?? '—'}</td>
      <td className="is-size-7"><AnalyticsToggleCell repoId={post.repo_id} postFolder={post.post_folder} sentAt={post.sent_at} /></td>
      <td className="is-size-7">
        {viewLinks.length > 0 ? viewLinks.map((l) => (
          <button key={l.platform} onClick={() => onOpenLink(l.url)} aria-label={`View ${l.platform} post`} className="button is-ghost is-small has-text-link">{l.platform} ↗</button>
        )) : '—'}
      </td>
    </tr>
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

  if (loading) return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%' }}>
      <p className="is-size-7 has-text-grey">Loading…</p>
    </div>
  );

  const sentPosts = posts.filter((p) => p.status === 'sent');
  const showModelBar = sentPosts.length >= 10 && stats.length > 0;

  if (posts.length === 0) return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%', padding: '2rem' }}>
      <p className="has-text-centered is-size-7 has-text-grey">No posts published yet. Draft your first post with <code>/draft-post</code> in your IDE.</p>
    </div>
  );

  return (
    <div className="p-5">
      <div className="is-flex is-align-items-center is-justify-content-space-between mb-5">
        <h1 className="has-text-weight-semibold">All repos — Published</h1>
        <div className="is-flex is-align-items-center" style={{ gap: '0.75rem' }}>
          {exportStatus && <span className="is-size-7 has-text-grey">{exportStatus}</span>}
          <button className="button is-outlined is-small" onClick={handleExport}>Export CSV</button>
        </div>
      </div>
      {showModelBar && <ModelBar stats={stats} />}
      <table className="table is-fullwidth is-striped is-narrow is-hoverable">
        <thead>
          <tr>
            <th>Repo</th><th>Slug</th><th>Sent</th>
            <th>Platforms</th><th>Scheduler</th><th>Model</th>
            <th>Engagement</th><th>Links</th>
          </tr>
        </thead>
        <tbody>
          {sentPosts.map((post) => <PublishedPostRow key={`${post.repo_id}-${post.post_folder}`} post={post} onOpenLink={handleOpenLink} onNavigateToRepo={onNavigateToRepo} />)}
        </tbody>
      </table>
      {hasMore && <div className="mt-4 has-text-centered"><button className="button is-outlined is-small" onClick={loadNextPage}>Load more</button></div>}
    </div>
  );
}
