// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '../ipc/invoke';
import { useAsyncCommand } from '../hooks/useAsyncCommand';
import type { PostAnalytics } from '../types';

interface Props {
  repoId: string;
  postFolder: string;
  sentAt?: string | null;
}

export function AnalyticsToggleCell({ repoId, postFolder, sentAt }: Props) {
  const [analytics, setAnalytics] = useState<PostAnalytics | null>(null);
  const { loading, run } = useAsyncCommand();
  const [triggered, setTriggered] = useState(false);

  async function handleLoad() {
    setTriggered(true);
    const data = await run(() => invoke<PostAnalytics>('get_post_analytics', { repoId, postFolder }));
    if (data !== null) {
      setAnalytics(data);
    }
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
