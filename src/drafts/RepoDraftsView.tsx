// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import PostCard from './PostCard';
import type { DraftPost, MetaChangedPayload } from '../types';

interface Props {
  repoId: string;
}

export default function RepoDraftsView({ repoId }: Props) {
  const [posts, setPosts] = useState<DraftPost[]>([]);
  const [repoName, setRepoName] = useState<string>('');
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const all = await invoke<DraftPost[]>('get_all_drafts');
      const filtered = all
        .filter((p) => p.repo_id === repoId)
        .sort((a, b) => {
          if (a.status === 'failed' && b.status !== 'failed') return -1;
          if (b.status === 'failed' && a.status !== 'failed') return 1;
          return (b.created_at ?? '').localeCompare(a.created_at ?? '');
        });
      setPosts(filtered);
      if (filtered.length > 0) setRepoName(filtered[0].repo_name);
    } catch (e) {
      console.error('get_all_drafts failed:', e);
    } finally {
      setLoading(false);
    }
  }, [repoId]);

  useEffect(() => { refresh(); }, [refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<MetaChangedPayload>('meta-changed', (event) => {
      if (event.payload.repo_id === repoId) refresh();
    })
      .then((fn) => { unlisten = fn; })
      .catch(console.error);
    return () => { unlisten?.(); };
  }, [repoId, refresh]);

  if (loading) return <div className="flex h-full items-center justify-center"><p className="text-sm text-zinc-400">Loading…</p></div>;

  return (
    <>
      <div className="border-b border-zinc-200 px-6 py-4 dark:border-zinc-700">
        <h1 className="text-base font-semibold text-zinc-900 dark:text-zinc-100">
          {repoName}
        </h1>
      </div>

      {posts.length === 0 ? (
        <div className="flex h-full items-center justify-center p-8">
          <p className="text-center text-sm text-zinc-500">No drafts waiting.<br />Invoke <code className="font-mono">/draft-post</code> in your IDE to create one.</p>
        </div>
      ) : (
        <div className="space-y-3 p-6">
          {posts.map((post) => (
            <PostCard key={`${post.repo_id}-${post.post_folder}`} post={post} onApproved={refresh} onDismissed={refresh} />
          ))}
        </div>
      )}
    </>
  );
}
