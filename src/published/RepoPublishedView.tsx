// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTimezone, formatTimestamp } from '../TimezoneContext';
import type { PublishedPost, PostAnalytics } from '../types';
import { isPublishedPost } from '../ipc-guards';

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
    <tr>
      <td className="is-family-monospace is-size-7">{post.post_folder}</td>
      <td className="is-size-7">{post.platforms.join(', ')}</td>
      <td className="is-size-7">{formatTimestamp(post.schedule, tz)}</td>
      <td className="is-size-7">
        {cancelError
          ? <span className="has-text-grey is-size-7">{cancelError}</span>
          : <button className="button is-outlined is-small" onClick={handleCancel} disabled={cancelling}>{cancelling ? 'Cancelling…' : 'Cancel'}</button>}
      </td>
    </tr>
  );
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
    <tr>
      <td className="is-family-monospace is-size-7">{post.post_folder}</td>
      <td className="has-text-grey is-size-7">{formatTimestamp(post.sent_at, tz)}</td>
      <td className="is-size-7">{sentPlatforms.join(', ')}</td>
      <td className="is-size-7">{post.llm_model ?? '—'}</td>
      <td className="is-size-7"><AnalyticsToggleCell repoId={post.repo_id} postFolder={post.post_folder} sentAt={post.sent_at} /></td>
      <td className="is-size-7">
        {viewLinks.length > 0 ? viewLinks.map((l) => (
          <button key={l.platform} onClick={() => handleOpenLink(l.url)} aria-label={`View ${l.platform} post`} className="button is-ghost is-small has-text-link">{l.platform} ↗</button>
        )) : '—'}
      </td>
    </tr>
  );
}

const QUEUED_POLL_MS = 30_000;

function useRepoPublished(repoId: string) {
  const [posts, setPosts] = useState<PublishedPost[]>([]);
  const [page, setPage] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);

  const loadPage = useCallback(async (pageIndex: number, append: boolean) => {
    try {
      const result = await invoke<unknown[]>('get_repo_published', { repoId, offset: pageIndex * PAGE_SIZE, limit: PAGE_SIZE + 1 });
      const filtered = result.filter(isPublishedPost);
      const hasMoreResults = filtered.length > PAGE_SIZE;
      const slice = hasMoreResults ? filtered.slice(0, PAGE_SIZE) : filtered;
      setPosts((prev) => (append ? [...prev, ...slice] : slice));
      setHasMore(hasMoreResults);
    } catch (e) { console.error('get_repo_published failed:', e); }
    finally { setLoading(false); }
  }, [repoId]);

  useEffect(() => { setPage(0); setPosts([]); setLoading(true); loadPage(0, false); }, [repoId, loadPage]);

  const hasQueued = posts.some((p) => p.status === 'queued');
  useEffect(() => {
    if (!hasQueued) return;
    const id = setInterval(() => loadPage(0, false), QUEUED_POLL_MS);
    return () => clearInterval(id);
  }, [hasQueued, loadPage]);

  const loadMore = () => { const next = page + 1; setPage(next); loadPage(next, true); };
  const refresh = useCallback(() => loadPage(0, false), [loadPage]);

  return { posts, hasMore, loading, loadMore, refresh };
}

export default function RepoPublishedView({ repoId }: Props) {
  const tz = useTimezone();
  const { posts, hasMore, loading, loadMore, refresh } = useRepoPublished(repoId);

  if (loading) return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%' }}>
      <p className="is-size-7 has-text-grey">Loading…</p>
    </div>
  );

  const queued = posts.filter((p) => p.status === 'queued');
  const sent = posts.filter((p) => p.status === 'sent');

  if (posts.length === 0) return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%', padding: '2rem' }}>
      <p className="has-text-centered is-size-7 has-text-grey">No posts sent yet. Draft your first post with <code>/draft-post</code> in your IDE.</p>
    </div>
  );

  return (
    <div className="p-5" style={{ display: 'flex', flexDirection: 'column', gap: '2rem' }}>
      {queued.length > 0 && (
        <section>
          <h2 className="has-text-grey is-size-7 has-text-weight-semibold mb-3">Scheduled</h2>
          <table className="table is-fullwidth is-narrow is-hoverable">
            <thead><tr><th>Post</th><th>Platforms</th><th>Scheduled for</th><th></th></tr></thead>
            <tbody>{queued.map((post) => <ScheduledRow key={post.post_folder} post={post} tz={tz} onCancelled={refresh} />)}</tbody>
          </table>
        </section>
      )}
      {sent.length > 0 && (
        <section>
          <table className="table is-fullwidth is-narrow is-hoverable">
            <thead><tr><th>Slug</th><th>Sent</th><th>Platforms</th><th>Model</th><th>Engagement</th><th>Links</th></tr></thead>
            <tbody>{sent.map((post) => <SentRow key={post.post_folder} post={post} tz={tz} />)}</tbody>
          </table>
          {hasMore && <div className="mt-4 has-text-centered"><button className="button is-outlined is-small" onClick={loadMore}>Load more</button></div>}
        </section>
      )}
    </div>
  );
}
