// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '../ipc/invoke';
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
  const [error, setError] = useState<string | null>(null);
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
      setError(null);
      if (filtered.length > 0) setRepoName(filtered[0].repo_name);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
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
    posts, repoName, loading, error, showSchedulerSetup, schedulerWarning, refresh,
    handleSetupLater: () => { setShowSchedulerSetup(false); setSchedulerWarning(true); },
    handleSchedulerDone: () => setShowSchedulerSetup(false),
    handleOpenSchedulerSettings: () => onOpenSchedulerSettings?.(),
  };
}

function DraftsError({ message }: { message: string }) {
  return (
    <div role="alert" className="notification is-danger is-light mx-5 mt-5 is-size-7">
      Failed to load drafts: {message}
    </div>
  );
}

export default function RepoDraftsView({ repoId, onOpenSchedulerSettings }: Props) {
  const {
    posts, repoName, loading, error, showSchedulerSetup, schedulerWarning, refresh,
    handleSetupLater, handleSchedulerDone, handleOpenSchedulerSettings,
  } = useRepoDraftsState(repoId, onOpenSchedulerSettings);

  if (loading) {
    return (
      <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%' }}>
        <p className="is-size-7 has-text-grey">Loading…</p>
      </div>
    );
  }
  if (error) return <DraftsError message={error} />;

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

      <div className="px-5 py-4" style={{ borderBottom: '1px solid var(--bulma-border-weak)' }}>
        <h1 className="has-text-weight-semibold">{repoName}</h1>
      </div>

      {schedulerWarning && (
        <div role="alert" className="notification is-warning is-light mx-5 mt-4 is-size-7" style={{ padding: '0.75rem 1rem' }}>
          No scheduler configured. Posts will fail until you add one in{' '}
          <strong>Settings → Scheduler</strong>.
        </div>
      )}

      {posts.length === 0 ? (
        <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100%', padding: '2rem' }}>
          <p className="has-text-centered is-size-7 has-text-grey">
            No drafts waiting.
            <br />
            Invoke <code>/draft-post</code> in your IDE to create one.
          </p>
        </div>
      ) : (
        <div className="p-5" style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
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
