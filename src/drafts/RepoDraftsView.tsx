// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import PostCard from './PostCard';
import SchedulerSetupModal from '../scheduling/SchedulerSetupModal';
import type { DraftPost, MetaChangedPayload } from '../types';

interface Props {
  repoId: string;
  onOpenSchedulerSettings?: () => void;
}

function useRepoDraftsState(repoId: string, onOpenSchedulerSettings?: () => void) {
  const [posts, setPosts] = useState<DraftPost[]>([]);
  const [repoName, setRepoName] = useState<string>('');
  const [loading, setLoading] = useState(true);
  const [showSchedulerSetup, setShowSchedulerSetup] = useState(false);
  const [schedulerWarning, setSchedulerWarning] = useState(false);

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
    invoke<boolean>('has_scheduler_configured', { repoId })
      .then((configured) => { if (!configured) setShowSchedulerSetup(true); })
      .catch(() => { /* keyring unavailable — don't block UI */ });
  }, [repoId]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<MetaChangedPayload>('meta-changed', (event) => {
      if (event.payload.repo_id === repoId) refresh();
    })
      .then((fn) => { unlisten = fn; })
      .catch(console.error);
    return () => { unlisten?.(); };
  }, [repoId, refresh]);

  return {
    posts, repoName, loading, showSchedulerSetup, schedulerWarning, refresh,
    handleSetupLater: () => { setShowSchedulerSetup(false); setSchedulerWarning(true); },
    handleSchedulerDone: () => setShowSchedulerSetup(false),
    handleOpenSchedulerSettings: () => onOpenSchedulerSettings?.(),
  };
}

export default function RepoDraftsView({ repoId, onOpenSchedulerSettings }: Props) {
  const {
    posts, repoName, loading, showSchedulerSetup, schedulerWarning, refresh,
    handleSetupLater, handleSchedulerDone, handleOpenSchedulerSettings,
  } = useRepoDraftsState(repoId, onOpenSchedulerSettings);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-sm text-zinc-400">Loading…</p>
      </div>
    );
  }

  return (
    <>
      {showSchedulerSetup && (
        <SchedulerSetupModal
          repoName={repoName}
          repoId={repoId}
          onSetupLater={handleSetupLater}
          onOpenSchedulerSettings={onOpenSchedulerSettings ? handleOpenSchedulerSettings : undefined}
          onDone={handleSchedulerDone}
        />
      )}

      <div className="border-b border-zinc-200 px-6 py-4 dark:border-zinc-700">
        <h1 className="text-base font-semibold text-zinc-900 dark:text-zinc-100">
          {repoName}
        </h1>
      </div>

      {schedulerWarning && (
        <div
          role="alert"
          className="mx-6 mt-4 rounded-md bg-amber-50 px-4 py-3 text-sm text-amber-800 dark:bg-amber-900/20 dark:text-amber-300"
        >
          No scheduler configured. Posts will fail until you add one in{' '}
          <strong>Settings → Scheduler</strong>.
        </div>
      )}

      {posts.length === 0 ? (
        <div className="flex h-full items-center justify-center p-8">
          <p className="text-center text-sm text-zinc-500">
            No drafts waiting.
            <br />
            Invoke <code className="font-mono">/draft-post</code> in your IDE to create one.
          </p>
        </div>
      ) : (
        <div className="space-y-3 p-6">
          {posts.map((post) => (
            <PostCard
              key={`${post.repo_id}-${post.post_folder}`}
              post={post}
              onApproved={refresh}
              onDismissed={refresh}
            />
          ))}
        </div>
      )}
    </>
  );
}
