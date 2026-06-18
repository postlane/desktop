// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '../ipc/invoke';
import { useTimezone, formatTimestamp } from '../TimezoneContext';
import type { PublishedPost } from '../types';
import { isPublishedPost } from '../ipc-guards';
import { AnalyticsToggleCell } from './AnalyticsCell';

const PAGE_SIZE = 100;

interface Props {
  onNavigateToRepo: (_repoId: string) => void;
}

function useAllPublished() {
  const [posts, setPosts] = useState<PublishedPost[]>([]);
  const [page, setPage] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadPage = useCallback(async (pageIndex: number, append: boolean) => {
    try {
      const raw = await invoke<unknown[]>('get_all_published', { offset: pageIndex * PAGE_SIZE, limit: PAGE_SIZE + 1 });
      const hasMoreResults = raw.length > PAGE_SIZE;
      const slice = raw.slice(0, PAGE_SIZE).filter(isPublishedPost);
      setPosts((prev) => (append ? [...prev, ...slice] : slice));
      setHasMore(hasMoreResults);
      setError(null);
    } catch (e) {
      setError(`Failed to load published posts: ${e instanceof Error ? e.message : String(e)}`);
    }
    setLoading(false);
  }, []);

  useEffect(() => { setPage(0); setPosts([]); setLoading(true); loadPage(0, false); }, [loadPage]);

  const loadNextPage = useCallback(() => {
    const next = page + 1; setPage(next); loadPage(next, true);
  }, [page, loadPage]);

  return { posts, hasMore, loading, error, loadNextPage };
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
  const [openLinkError, setOpenLinkError] = useState<string | null>(null);
  const { posts, hasMore, loading, error, loadNextPage } = useAllPublished();

  async function handleExport() {
    setExportStatus(null);
    try { const path = await invoke<string>('export_history_csv'); setExportStatus(`Saved to ${path}`); }
    catch (e) { setExportStatus(e instanceof Error ? e.message : 'Export failed'); }
  }

  async function handleOpenLink(url: string) {
    setOpenLinkError(null);
    try { await invoke('plugin:opener|open_url', { url }); }
    catch (e) { setOpenLinkError(e instanceof Error ? e.message : 'Failed to open link'); }
  }

  if (loading) return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%' }}>
      <p className="is-size-7 has-text-grey">Loading…</p>
    </div>
  );

  if (error) return (
    <div role="alert" className="notification is-danger is-light mx-5 mt-5 is-size-7">
      {error}
    </div>
  );

  const sentPosts = posts.filter((p) => p.status === 'sent');

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
          {openLinkError && <span className="is-size-7 has-text-danger">{openLinkError}</span>}
          {exportStatus && <span className="is-size-7 has-text-grey">{exportStatus}</span>}
          <button className="button is-outlined is-small" onClick={handleExport}>Export CSV</button>
        </div>
      </div>
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
