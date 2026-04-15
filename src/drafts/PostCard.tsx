// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback } from 'react';
import { useTimezone, formatTimestamp } from '../TimezoneContext';
import { invoke } from '@tauri-apps/api/core';
import {
  ChevronDownIcon,
  XMarkIcon,
} from '@heroicons/react/20/solid';
import { Button } from '../components/catalyst/button';
import { Badge } from '../components/catalyst/badge';
import type { DraftPost, Platform } from '../types';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface Props {
  post: DraftPost;
  onApproved: () => void;
  onDismissed: () => void;
  isFocused?: boolean;
}

const PLATFORM_LABELS: Record<string, string> = {
  x: 'X',
  bluesky: 'Bluesky',
  mastodon: 'Mastodon',
};

const PLATFORM_ORDER: Platform[] = ['x', 'bluesky', 'mastodon'];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function triggerText(post: DraftPost): string {
  if (post.trigger) return post.trigger;
  return post.post_folder.slice(0, 80);
}

function platformsOnPost(post: DraftPost): Platform[] {
  return PLATFORM_ORDER.filter((p) => post.platforms.includes(p));
}

// ---------------------------------------------------------------------------
// Platform tabs
// ---------------------------------------------------------------------------

function PlatformTabs({
  platforms,
  active,
  onChange,
}: {
  platforms: Platform[];
  active: Platform;
  onChange: (p: Platform) => void;
}) {
  return (
    <div role="tablist" className="flex gap-1 border-b border-zinc-200 dark:border-zinc-700">
      {platforms.map((p, i) => (
        <button
          key={p}
          role="tab"
          aria-selected={p === active}
          onClick={() => onChange(p)}
          data-slot="tab"
          className={[
            'px-3 py-1.5 text-sm font-medium border-b-2 -mb-px focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500',
            p === active
              ? 'border-zinc-900 text-zinc-900 dark:border-zinc-100 dark:text-zinc-100'
              : 'border-transparent text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300',
          ].join(' ')}
          aria-label={`${PLATFORM_LABELS[p] ?? p} (${i + 1})`}
        >
          {PLATFORM_LABELS[p] ?? p}
        </button>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Platform results row (for failed cards)
// ---------------------------------------------------------------------------

function PlatformResults({ results }: { results: Record<string, string> }) {
  return (
    <div className="flex flex-wrap gap-2">
      {Object.entries(results).map(([platform, result]) => (
        <span key={platform} className="flex items-center gap-1 text-xs">
          <span className="capitalize text-zinc-600 dark:text-zinc-400">{platform}</span>
          {result === 'sent' || result === 'success'
            ? <span className="text-green-600">✓</span>
            : <span className="text-red-600">✗</span>}
        </span>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// PostCard
// ---------------------------------------------------------------------------

export default function PostCard({ post, onApproved, onDismissed, isFocused = false }: Props) {
  const tz = useTimezone();
  const isFailed = post.status === 'failed';
  const [expanded, setExpanded] = useState(isFailed);
  const [activeTab, setActiveTab] = useState<Platform>(
    (post.platforms[0] as Platform) ?? 'x',
  );
  const [approving, setApproving] = useState(false);
  const [approveError, setApproveError] = useState<string | null>(null);
  const [retrying, setRetrying] = useState(false);

  const platforms = platformsOnPost(post);

  const handleApprove = useCallback(async () => {
    setApproving(true);
    setApproveError(null);
    try {
      await invoke('approve_post', {
        repoPath: post.repo_path,
        postFolder: post.post_folder,
      });
      onApproved();
    } catch (e) {
      setApproveError(e instanceof Error ? e.message : String(e));
    } finally {
      setApproving(false);
    }
  }, [post, onApproved]);

  const handleDismiss = useCallback(async () => {
    try {
      await invoke('dismiss_post', {
        repoPath: post.repo_path,
        postFolder: post.post_folder,
      });
      onDismissed();
    } catch (e) {
      console.error('dismiss_post failed:', e);
    }
  }, [post, onDismissed]);

  const handleRetry = useCallback(async () => {
    setRetrying(true);
    try {
      await invoke('retry_post', {
        repoPath: post.repo_path,
        postFolder: post.post_folder,
      });
      onApproved();
    } catch (e) {
      console.error('retry_post failed:', e);
    } finally {
      setRetrying(false);
    }
  }, [post, onApproved]);

  // No global keydown listener — handled inline via onKeyDown on the card div

  const focusClass = isFocused
    ? 'ring-2 ring-blue-500 bg-blue-50/40 dark:bg-blue-900/10'
    : '';

  return (
    <article
      role="article"
      data-post-card
      onKeyDown={(e) => {
        if (!isFocused) return;
        switch (e.key.toLowerCase()) {
          case 'a': e.preventDefault(); handleApprove(); break;
          case 'd': e.preventDefault(); handleDismiss(); break;
          case 'e': e.preventDefault(); setExpanded((v) => !v); break;
          case 'r': if (isFailed) { e.preventDefault(); handleRetry(); } break;
          case '1': setActiveTab(platforms[0] ?? 'x'); break;
          case '2': if (platforms[1]) setActiveTab(platforms[1]); break;
          case '3': if (platforms[2]) setActiveTab(platforms[2]); break;
          case 'escape': e.preventDefault(); setExpanded(false); break;
        }
      }}
      tabIndex={0}
      className={`rounded-xl border border-zinc-200 bg-white p-4 dark:border-zinc-700 dark:bg-zinc-900 focus:outline-none ${focusClass}`}
    >

      {/* Header row */}
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          {/* Repo badge + status */}
          <div className="mb-1 flex items-center gap-2">
            <Badge color="zinc">{post.repo_name}</Badge>
            {isFailed && <Badge color="red">Failed</Badge>}
          </div>

          {/* Trigger text */}
          <p className="truncate text-sm font-medium text-zinc-900 dark:text-zinc-100">
            {triggerText(post)}
          </p>

          {/* Platforms + schedule */}
          <p className="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
            {post.platforms.join(' · ')}
            {post.schedule && (
              <> · {formatTimestamp(post.schedule, tz)}</>
            )}
          </p>
        </div>

        {/* Actions */}
        <div className="flex shrink-0 items-center gap-2">
          {isFailed ? (
            <Button
              onClick={handleRetry}
              disabled={retrying}
              color="amber"
            >
              {retrying ? 'Retrying…' : 'Retry'}
            </Button>
          ) : (
            <Button
              onClick={handleApprove}
              disabled={approving}
              color="green"
            >
              {approving ? 'Approving…' : 'Approve'}
            </Button>
          )}

          <Button
            plain
            onClick={() => setExpanded((v) => !v)}
            aria-label="Preview"
            aria-expanded={expanded}
          >
            <ChevronDownIcon
              className={`h-4 w-4 transition-transform ${expanded ? 'rotate-180' : ''}`}
            />
            Preview
          </Button>

          <Button plain onClick={handleDismiss} aria-label="Dismiss">
            <XMarkIcon className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {/* Approve error */}
      {approveError && (
        <p className="mt-2 text-xs text-red-600 dark:text-red-400">{approveError}</p>
      )}

      {/* Failed: error message + platform results */}
      {isFailed && post.error && (
        <div className="mt-3 rounded-lg bg-red-50 p-3 dark:bg-red-900/20">
          <p className="text-xs text-red-700 dark:text-red-400">{post.error}</p>
          {post.platform_results && (
            <div className="mt-2">
              <PlatformResults results={post.platform_results} />
            </div>
          )}
        </div>
      )}

      {/* Expanded: platform tabs + preview */}
      {expanded && platforms.length > 0 && (
        <div className="mt-4">
          <PlatformTabs
            platforms={platforms}
            active={activeTab}
            onChange={setActiveTab}
          />
          <div className="mt-3 rounded-lg bg-zinc-50 p-3 dark:bg-zinc-800">
            <p className="text-xs text-zinc-500 dark:text-zinc-400">
              {PLATFORM_LABELS[activeTab] ?? activeTab} preview — post content renders here (5.9)
            </p>
          </div>
          {post.image_url && (
            <img
              src={post.image_url}
              alt="Post image"
              className="mt-3 h-32 w-auto rounded-lg object-cover"
            />
          )}
        </div>
      )}
    </article>
  );
}
