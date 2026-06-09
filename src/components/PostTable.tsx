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
  /** Connected platform slugs keyed by repo ID. Undefined = not yet loaded (show all). */
  connectedPlatformsByRepo?: Record<string, string[]>;
  /** Called when user clicks a greyed-out (disconnected) platform badge. */
  onConnectPlatform?: () => void;
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
//
// Badge rendering states — must remain visually distinct:
//   connected + status="failed" → platform colour button + red dot overlay (failed-indicator)
//   connected + other status    → platform colour button, no overlay
//   disconnected platform       → grey/dim button (post-row-disconnected), no overlay
//
// Never collapse these two: a failed post was sent to the platform and bounced;
// a disconnected platform was never configured. The user action is different.

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

const BTN_STYLE = { width: '2rem', height: '2rem', padding: 0, borderRadius: '50%', flexShrink: 0, border: 'none' } as const;

function PlatformButton({ post, onSelect, onConnectPlatform, isConnected }: {
  post: DraftPost;
  onSelect: (_post: DraftPost) => void;
  onConnectPlatform?: () => void;
  isConnected: boolean;
}) {
  const cfg = PLATFORM_CFG[post.platform];
  const name = cfg?.label ?? post.platform;
  const icon = PLATFORM_ICONS[post.platform ?? ''];
  const content = icon ? <FontAwesomeIcon icon={icon} /> : name;
  if (!isConnected) {
    return (
      <button key={post.platform} data-testid="post-row-disconnected"
        aria-label={`Connect ${name} to approve`} title={`Connect ${name} to approve`}
        onClick={() => onConnectPlatform?.()}
        className="button is-small"
        style={{ ...BTN_STYLE, background: 'hsl(0,0%,75%)', color: '#fff', opacity: 0.6 }}>
        {content}
      </button>
    );
  }
  const isFailed = post.status === 'failed';
  return (
    <button key={post.platform} data-testid="post-row"
      aria-label={`Edit ${name} post`} title={name}
      onClick={() => onSelect(post)}
      className="button is-small"
      style={{ ...BTN_STYLE, position: 'relative', background: cfg?.color ?? 'hsl(0,0%,50%)', color: '#fff' }}>
      {content}
      {isFailed && (
        <span data-testid="failed-indicator"
          style={{ position: 'absolute', top: 0, right: 0, width: '0.5rem', height: '0.5rem',
                   borderRadius: '50%', background: 'hsl(348,100%,61%)' /* Bulma danger */ }} />
      )}
    </button>
  );
}

function GroupCard({ group, onSelect, connectedPlatforms, onConnectPlatform }: {
  group: DraftGroup;
  onSelect: (_post: DraftPost) => void;
  connectedPlatforms: string[] | undefined;
  onConnectPlatform?: () => void;
}) {
  const firstPost = group.posts[0];
  const label = firstPost?.trigger ?? group.post_folder;
  return (
    <div style={{ borderLeft: '3px solid var(--bulma-link)', marginBottom: '0.75rem' }}>
      <div className="px-4 pt-2 pb-1 has-background-white-ter is-flex is-align-items-baseline" style={{ gap: '0.5rem' }}>
        <span data-testid="group-label" className="is-size-7 has-text-weight-medium">{label}</span>
        <span data-testid="group-repo-name" className="is-size-7 has-text-grey-light" style={{ flexShrink: 0 }}>{firstPost?.repo_name}</span>
      </div>
      <div className="px-4 py-2 has-background-white-ter is-flex is-align-items-center"
        style={{ gap: '0.5rem', borderTop: '1px solid var(--bulma-border-weak)' }}>
        {group.posts.map((post) => (
          <PlatformButton key={post.platform} post={post} onSelect={onSelect}
            onConnectPlatform={onConnectPlatform}
            isConnected={connectedPlatforms === undefined || connectedPlatforms.includes(post.platform)} />
        ))}
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

function QueueTable({ posts, onSelect, timezone: _timezone, connectedPlatformsByRepo, onConnectPlatform }: PostTableQueueProps) {
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
        <GroupCard key={group.key} group={group} onSelect={onSelect}
          connectedPlatforms={connectedPlatformsByRepo?.[group.posts[0]?.repo_id ?? '']}
          onConnectPlatform={onConnectPlatform} />
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
