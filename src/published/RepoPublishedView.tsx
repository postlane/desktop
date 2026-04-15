// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/catalyst/button';
import { useTimezone, formatTimestamp } from '../TimezoneContext';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../components/catalyst/table';
import type { PublishedPost } from '../types';

const PAGE_SIZE = 100;

interface Props {
  repoId: string;
}

// ---------------------------------------------------------------------------
// Scheduled sub-section
// ---------------------------------------------------------------------------

function ScheduledRow({ post, onCancelled, tz }: { post: PublishedPost; onCancelled: () => void; tz: string }) {
  const [cancelling, setCancelling] = useState(false);
  const [cancelError, setCancelError] = useState<string | null>(null);

  // Use first scheduler_id found
  const firstPlatform = post.platforms[0] ?? 'x';
  const postId = post.scheduler_ids?.[firstPlatform] ?? '';

  async function handleCancel() {
    setCancelling(true);
    setCancelError(null);
    try {
      await invoke('cancel_post_command', {
        repoPath: post.repo_path,
        postFolder: post.post_folder,
        postId,
        platform: firstPlatform,
      });
      onCancelled();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.toLowerCase().includes('not supported')) {
        setCancelError(`Cancel via dashboard`);
      } else {
        setCancelError(msg);
      }
    } finally {
      setCancelling(false);
    }
  }

  return (
    <TableRow>
      <TableCell className="font-mono text-xs">{post.post_folder}</TableCell>
      <TableCell>{post.platforms.join(', ')}</TableCell>
      <TableCell>{formatTimestamp(post.schedule, tz)}</TableCell>
      <TableCell>
        {cancelError ? (
          <span className="text-xs text-zinc-500">{cancelError}</span>
        ) : (
          <Button outline onClick={handleCancel} disabled={cancelling}>
            {cancelling ? 'Cancelling…' : 'Cancel'}
          </Button>
        )}
      </TableCell>
    </TableRow>
  );
}

// ---------------------------------------------------------------------------
// Sent posts table
// ---------------------------------------------------------------------------

function SentRow({ post, tz }: { post: PublishedPost; tz: string }) {
  const sentPlatforms = post.platform_results
    ? Object.entries(post.platform_results)
        .filter(([, v]) => v === 'sent')
        .map(([k]) => k)
    : post.platforms;

  // Build view links from scheduler_ids
  const viewLinks = sentPlatforms.map((platform) => {
    const id = post.scheduler_ids?.[platform];
    return { platform, id: id ?? null };
  });

  return (
    <TableRow>
      <TableCell className="font-mono text-xs">{post.post_folder}</TableCell>
      <TableCell className="text-xs text-zinc-500">
        {formatTimestamp(post.sent_at, tz)}
      </TableCell>
      <TableCell className="text-xs">{sentPlatforms.join(', ')}</TableCell>
      <TableCell className="text-xs">{post.llm_model ?? '—'}</TableCell>
      <TableCell className="text-xs text-zinc-400">—</TableCell>
      <TableCell className="text-xs">
        {viewLinks.some((l) => l.id)
          ? viewLinks.filter((l) => l.id).map((l) => (
              <span key={l.platform} className="mr-2">{l.platform}</span>
            ))
          : '—'}
      </TableCell>
    </TableRow>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export default function RepoPublishedView({ repoId }: Props) {
  const tz = useTimezone();
  const [posts, setPosts] = useState<PublishedPost[]>([]);
  const [page, setPage] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);

  const loadPage = useCallback(async (pageIndex: number, append: boolean) => {
    try {
      const result = await invoke<PublishedPost[]>('get_repo_published', {
        repoId,
        offset: pageIndex * PAGE_SIZE,
        limit: PAGE_SIZE + 1, // fetch one extra to detect more
      });
      const hasMoreResults = result.length > PAGE_SIZE;
      const slice = hasMoreResults ? result.slice(0, PAGE_SIZE) : result;
      setPosts((prev) => (append ? [...prev, ...slice] : slice));
      setHasMore(hasMoreResults);
    } catch (e) {
      console.error('get_repo_published failed:', e);
    } finally {
      setLoading(false);
    }
  }, [repoId]);

  useEffect(() => {
    setPage(0);
    setPosts([]);
    setLoading(true);
    loadPage(0, false);
  }, [repoId, loadPage]);

  function handleLoadMore() {
    const next = page + 1;
    setPage(next);
    loadPage(next, true);
  }

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-sm text-zinc-400">Loading…</p>
      </div>
    );
  }

  const queued = posts.filter((p) => p.status === 'queued');
  const sent = posts.filter((p) => p.status === 'sent');

  if (posts.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-8">
        <p className="text-center text-sm text-zinc-500">
          No posts sent yet. Draft your first post with{' '}
          <code className="font-mono">/draft-post</code> in your IDE.
        </p>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-8">
      {/* Scheduled sub-section */}
      {queued.length > 0 && (
        <section>
          <h2 role="heading" className="mb-3 text-sm font-semibold text-zinc-700 dark:text-zinc-300">
            Scheduled
          </h2>
          <Table>
            <TableHead>
              <TableRow>
                <TableHeader>Post</TableHeader>
                <TableHeader>Platforms</TableHeader>
                <TableHeader>Scheduled for</TableHeader>
                <TableHeader></TableHeader>
              </TableRow>
            </TableHead>
            <TableBody>
              {queued.map((post) => (
                <ScheduledRow
                  key={post.post_folder}
                  post={post}
                  tz={tz}
                  onCancelled={() => loadPage(0, false)}
                />
              ))}
            </TableBody>
          </Table>
        </section>
      )}

      {/* Sent posts table */}
      {sent.length > 0 && (
        <section>
          <Table>
            <TableHead>
              <TableRow>
                <TableHeader>Slug</TableHeader>
                <TableHeader>Sent</TableHeader>
                <TableHeader>Platforms</TableHeader>
                <TableHeader>Model</TableHeader>
                <TableHeader>Engagement</TableHeader>
                <TableHeader>Links</TableHeader>
              </TableRow>
            </TableHead>
            <TableBody>
              {sent.map((post) => (
                <SentRow key={post.post_folder} post={post} tz={tz} />
              ))}
            </TableBody>
          </Table>

          {hasMore && (
            <div className="mt-4 text-center">
              <Button outline onClick={handleLoadMore}>Load more</Button>
            </div>
          )}
        </section>
      )}
    </div>
  );
}
