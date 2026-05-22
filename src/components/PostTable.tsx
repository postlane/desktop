// SPDX-License-Identifier: BUSL-1.1

import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faXTwitter, faBluesky, faMastodon, faLinkedinIn } from '@fortawesome/free-brands-svg-icons';
import type { IconDefinition } from '@fortawesome/fontawesome-svg-core';
import { PLATFORM_CFG } from '../constants/platformConfig';
import { formatRelativeTime, formatScheduled } from '../formatting/timeFormat';
import type { DraftPost, PublishedPost } from '../types';

const PLATFORM_ICONS: Record<string, IconDefinition> = {
  x: faXTwitter,
  bluesky: faBluesky,
  mastodon: faMastodon,
  linkedin: faLinkedinIn,
};

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

function GroupCard({ group, onSelect }: {
  group: DraftGroup;
  onSelect: (_post: DraftPost) => void;
}) {
  const firstPost = group.posts[0];
  const label = firstPost?.trigger ?? group.post_folder;
  return (
    <div style={{ borderLeft: '3px solid var(--bulma-link)', marginBottom: '0.75rem' }}>
      <div data-testid="group-label" className="px-4 pt-2 pb-2 is-size-7 has-text-weight-medium has-background-white-ter">
        {label}
      </div>
      <div className="px-4 py-2 has-background-white-ter is-flex is-align-items-center"
        style={{ gap: '0.5rem', borderTop: '1px solid var(--bulma-border-weak)' }}>
        {group.posts.map((post) => {
          const cfg = PLATFORM_CFG[post.platform];
          const color = cfg?.color ?? 'hsl(0,0%,50%)';
          const name = cfg?.label ?? post.platform;
          const isFailed = post.status === 'failed';
          const icon = PLATFORM_ICONS[post.platform ?? ''];
          return (
            <button key={post.platform} data-testid="post-row"
              aria-label={`Edit ${name} post`} title={name}
              onClick={() => onSelect(post)}
              className={'button is-small' + (isFailed ? ' has-text-danger' : '')}
              style={{ width: '2rem', height: '2rem', padding: 0, borderRadius: '50%', flexShrink: 0, border: 'none',
                background: isFailed ? undefined : color, color: isFailed ? undefined : '#fff' }}>
              {icon ? <FontAwesomeIcon icon={icon} /> : name}
            </button>
          );
        })}
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

function QueueTable({ posts, onSelect, timezone: _timezone }: PostTableQueueProps) {
  if (posts.length === 0) {
    return (
      <div className="px-4 py-6 has-text-centered">
        <p className="is-size-7 has-text-grey">Queue is empty. Run /draft-post in your IDE to generate a post.</p>
      </div>
    );
  }
  const groups = groupDrafts(posts);
  return (
    <div className="px-4 pt-4">
      {groups.map((group) => (
        <GroupCard key={group.key} group={group} onSelect={onSelect} />
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
