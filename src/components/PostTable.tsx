// SPDX-License-Identifier: BUSL-1.1

import { PLATFORM_CFG } from '../constants/platformConfig';
import { formatRelativeTime, formatScheduled } from '../utils/timeFormat';
import type { DraftPost, PublishedPost } from '../types';

// ── Types ─────────────────────────────────────────────────────────────────────

type PostTableQueueProps = {
  posts: DraftPost[];
  isHistory: false;
  onSelect: (_post: DraftPost) => void;
  timezone: string;
};

type PostTableHistoryProps = {
  posts: PublishedPost[];
  isHistory: true;
  onSelect: (_post: PublishedPost) => void;
  timezone: string;
};

export type PostTableProps = PostTableQueueProps | PostTableHistoryProps;

interface DraftGroup {
  key: string;
  post_folder: string;
  posts: DraftPost[];
}

// ── Grouping ──────────────────────────────────────────────────────────────────

function groupDrafts(posts: DraftPost[]): DraftGroup[] {
  const groups = new Map<string, DraftGroup>();
  for (const post of posts) {
    const key = JSON.stringify([post.repo_path, post.post_folder]);
    const existing = groups.get(key);
    if (existing) {
      existing.posts.push(post);
    } else {
      groups.set(key, { key, post_folder: post.post_folder, posts: [post] });
    }
  }
  return Array.from(groups.values());
}

// ── Sub-components ────────────────────────────────────────────────────────────

function PlatformBadge({ platform }: { platform: string }) {
  const cfg = PLATFORM_CFG[platform];
  const label = cfg?.label ?? platform;
  const color = cfg?.color ?? 'hsl(0, 0%, 50%)';
  return (
    <span className="tag is-rounded is-small" style={{ background: color, color: '#fff' }}>
      {label}
    </span>
  );
}

function QueueRow({ post, isFirstInGroup, onSelect, timezone }: {
  post: DraftPost;
  isFirstInGroup: boolean;
  onSelect: (_post: DraftPost) => void;
  timezone: string;
}) {
  const timeLabel = post.scheduled_for
    ? formatScheduled(post.scheduled_for, timezone)
    : formatRelativeTime(post.created_at);
  const rowClass = [
    'post-row px-4 py-2 is-clickable has-background-white-ter',
    post.status === 'failed' ? 'has-text-danger' : '',
  ].join(' ').trim();
  return (
    <div data-testid="post-row" className={rowClass} style={{ borderLeft: '3px solid var(--bulma-link)', cursor: 'pointer' }}
      onClick={() => onSelect(post)} role="button" tabIndex={0}
      onKeyDown={(e) => { if (e.key === 'Enter') onSelect(post); }}>
      {isFirstInGroup && (
        <div data-testid="group-label" className="is-size-7 has-text-grey mb-1">{post.post_folder}</div>
      )}
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <PlatformBadge platform={post.platform} />
        <span className="is-size-7 has-text-grey">{timeLabel}</span>
      </div>
    </div>
  );
}

function HistoryRow({ post, timezone, onSelect }: {
  post: PublishedPost;
  timezone: string;
  onSelect: (_post: PublishedPost) => void;
}) {
  const platform = post.platform ?? '';
  const timeLabel = post.sent_at ? formatRelativeTime(post.sent_at) : formatScheduled(post.sent_at ?? '', timezone);
  return (
    <div data-testid="post-row" className="px-4 py-2 is-clickable" style={{ cursor: 'pointer', borderBottom: '1px solid var(--bulma-border-weak)' }}
      onClick={() => onSelect(post)} role="button" tabIndex={0}
      onKeyDown={(e) => { if (e.key === 'Enter') onSelect(post); }}>
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <PlatformBadge platform={platform} />
        <span className="is-size-7 has-text-grey">{timeLabel}</span>
        <span className="is-size-7 has-text-grey is-size-7">{post.post_folder}</span>
      </div>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

function QueueTable({ posts, onSelect, timezone }: PostTableQueueProps) {
  if (posts.length === 0) {
    return (
      <div className="px-4 py-6 has-text-centered">
        <p className="is-size-7 has-text-grey">Queue is empty. Run /draft-post in your IDE to generate a post.</p>
      </div>
    );
  }
  const groups = groupDrafts(posts);
  return (
    <div>
      {groups.map((group) => (
        <div key={group.key}>
          {group.posts.map((post, idx) => (
            <QueueRow key={`${post.platform}`} post={post} isFirstInGroup={idx === 0}
              onSelect={onSelect} timezone={timezone} />
          ))}
        </div>
      ))}
    </div>
  );
}

function HistoryTable({ posts, onSelect, timezone }: PostTableHistoryProps) {
  if (posts.length === 0) {
    return (
      <div className="px-4 py-6 has-text-centered">
        <p className="is-size-7 has-text-grey">No posts sent yet.</p>
      </div>
    );
  }
  return (
    <div>
      {posts.map((post, idx) => (
        <HistoryRow key={idx} post={post} timezone={timezone} onSelect={onSelect} />
      ))}
    </div>
  );
}

export default function PostTable(props: PostTableProps) {
  if (props.isHistory) return <HistoryTable {...props} />;
  return <QueueTable {...props} />;
}
