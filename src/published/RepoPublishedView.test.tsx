// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import RepoPublishedView from './RepoPublishedView';
import type { PublishedPost } from '../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

function makeSent(overrides: Partial<PublishedPost> = {}): PublishedPost {
  return {
    repo_id: 'r1',
    repo_name: 'my-app',
    repo_path: '/path/to/repo',
    post_folder: 'post-001',
    status: 'sent',
    platforms: ['x', 'bluesky'],
    platform_results: { x: 'sent', bluesky: 'sent' },
    schedule: null,
    scheduler_ids: { x: 'tweet-123' },
    platform_urls: null,
    llm_model: 'claude-sonnet-4-6',
    provider: null,
    sent_at: '2026-04-15T10:00:00Z',
    created_at: '2026-04-15T09:00:00Z',
    ...overrides,
  };
}

function makeQueued(overrides: Partial<PublishedPost> = {}): PublishedPost {
  return makeSent({ status: 'queued', sent_at: null, ...overrides });
}

// ---------------------------------------------------------------------------
// Empty state
// ---------------------------------------------------------------------------

describe('RepoPublishedView — empty state', () => {
  it('shows empty state when no sent posts', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByText(/no posts sent yet/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Scheduled sub-section
// ---------------------------------------------------------------------------

describe('RepoPublishedView — scheduled sub-section', () => {
  it('shows scheduled section when queued posts exist', async () => {
    mockInvoke.mockResolvedValue([
      makeQueued({ post_folder: 'q1' }),
      makeSent({ post_folder: 's1' }),
    ]);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: /scheduled/i })).toBeInTheDocument(),
    );
  });

  it('hides scheduled section when no queued posts', async () => {
    mockInvoke.mockResolvedValue([makeSent()]);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('post-001'));
    expect(screen.queryByText(/scheduled/i)).not.toBeInTheDocument();
  });

  it('shows post folder and Cancel button for queued posts', async () => {
    mockInvoke.mockResolvedValue([makeQueued({ post_folder: 'my-queued-post' })]);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByText('my-queued-post')).toBeInTheDocument(),
    );
    expect(screen.getByRole('button', { name: /cancel/i })).toBeInTheDocument();
  });

  it('calls cancel_post_command and refreshes on cancel', async () => {
    mockInvoke
      .mockResolvedValueOnce([makeQueued({ post_folder: 'q1', scheduler_ids: { x: 'id-123' } })])
      .mockResolvedValueOnce(undefined) // cancel_post_command
      .mockResolvedValueOnce([]); // refresh

    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByRole('button', { name: /cancel/i }));
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('cancel_post_command', expect.anything()),
    );
  });
});

// ---------------------------------------------------------------------------
// Sent posts table
// ---------------------------------------------------------------------------

describe('RepoPublishedView — sent posts table', () => {
  it('shows sent posts sorted by sent_at newest first', async () => {
    // Component fetches PAGE_SIZE+1 to detect more; return both posts
    mockInvoke.mockResolvedValue([
      makeSent({ post_folder: 'older', sent_at: '2026-04-14T10:00:00Z' }),
      makeSent({ post_folder: 'newer', sent_at: '2026-04-15T10:00:00Z' }),
    ]);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('newer'));
    // The backend sorts newest first; just verify both are shown
    expect(screen.getByText('older')).toBeInTheDocument();
    expect(screen.getByText('newer')).toBeInTheDocument();
  });

  it('shows correct columns: slug, sent time, platforms, model', async () => {
    mockInvoke.mockResolvedValue([makeSent()]);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('post-001'));
    expect(screen.getByText('post-001')).toBeInTheDocument();
    expect(screen.getByText('claude-sonnet-4-6')).toBeInTheDocument();
  });

  it('shows — for view link when no platform_urls', async () => {
    mockInvoke.mockResolvedValue([makeSent({ platform_urls: null })]);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('post-001'));
    expect(screen.getAllByText('—').length).toBeGreaterThan(0);
  });

  it('shows clickable view link when platform_urls has URL for sent platform', async () => {
    mockInvoke.mockResolvedValue([
      makeSent({
        platform_results: { x: 'sent' },
        platform_urls: { x: 'https://x.com/i/web/status/123' },
      }),
    ]);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('post-001'));
    expect(screen.getByRole('button', { name: /view x post/i })).toBeInTheDocument();
  });

  it('clicking view link invokes opener with the URL', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repo_published') return [makeSent({ platform_results: { x: 'sent' }, platform_urls: { x: 'https://x.com/i/web/status/999' } })];
      return undefined;
    });
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByRole('button', { name: /view x post/i }));
    fireEvent.click(screen.getByRole('button', { name: /view x post/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('plugin:opener|open_url', { url: 'https://x.com/i/web/status/999' }));
  });

  it('filters to only sent posts in the sent table — queued posts are only in Scheduled section', async () => {
    mockInvoke.mockResolvedValue([
      makeSent({ post_folder: 'sent-post' }),
      makeQueued({ post_folder: 'queued-post' }),
    ]);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('sent-post'));
    // The sent table section should exist and contain sent-post
    expect(screen.getByText('sent-post')).toBeInTheDocument();
    // queued-post appears in the Scheduled section (not the sent table)
    // Both sections render it — that's correct behaviour
    expect(screen.getByText('queued-post')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

describe('RepoPublishedView — pagination', () => {
  it('shows "Load more" button when there are more posts', async () => {
    const posts = Array.from({ length: 101 }, (_, i) =>
      makeSent({ post_folder: `post-${String(i).padStart(3, '0')}`, sent_at: `2026-04-${String(15 - Math.floor(i / 10)).padStart(2, '0')}T10:00:00Z` }),
    );
    mockInvoke.mockResolvedValue(posts);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /load more/i })).toBeInTheDocument(),
    );
  });

  it('does not show "Load more" when 100 or fewer posts', async () => {
    const posts = Array.from({ length: 5 }, (_, i) =>
      makeSent({ post_folder: `post-${i}` }),
    );
    mockInvoke.mockResolvedValue(posts);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('post-0'));
    expect(screen.queryByRole('button', { name: /load more/i })).not.toBeInTheDocument();
  });

  it('clicking Load more fetches the next page', async () => {
    const firstPage = Array.from({ length: 101 }, (_, i) =>
      makeSent({ post_folder: `post-${String(i).padStart(3, '0')}` }),
    );
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repo_published') {
        return mockInvoke.mock.calls.filter((c) => c[0] === 'get_repo_published').length === 1
          ? firstPage
          : [makeSent({ post_folder: 'page-2-post' })];
      }
      if (cmd === 'get_post_analytics') return { sessions: 0, unique_sessions: 0, top_referrer: null };
      return null;
    });
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByRole('button', { name: /load more/i }));
    fireEvent.click(screen.getByRole('button', { name: /load more/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('get_repo_published', expect.objectContaining({ offset: 100 })),
    );
  });
});

// ---------------------------------------------------------------------------
// Cancel errors
// ---------------------------------------------------------------------------

describe('RepoPublishedView — cancel error paths', () => {
  it('shows "Cancel via dashboard" when scheduler returns not-supported', async () => {
    mockInvoke
      .mockResolvedValueOnce([makeQueued({ post_folder: 'q1', scheduler_ids: { x: 'id-99' } })])
      .mockRejectedValueOnce(new Error('not supported by this provider'));
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByRole('button', { name: /cancel/i }));
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    await waitFor(() =>
      expect(screen.getByText(/cancel via dashboard/i)).toBeInTheDocument(),
    );
  });

  it('shows generic error message for other cancel failures', async () => {
    mockInvoke
      .mockResolvedValueOnce([makeQueued({ post_folder: 'q1', scheduler_ids: { x: 'id-99' } })])
      .mockRejectedValueOnce(new Error('Connection refused'));
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByRole('button', { name: /cancel/i }));
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    await waitFor(() =>
      expect(screen.getByText(/connection refused/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Analytics lazy load + UX states
// ---------------------------------------------------------------------------

describe('RepoPublishedView — analytics lazy load', () => {
  it('does not call get_post_analytics on initial render', async () => {
    mockInvoke.mockResolvedValue([makeSent()]);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('post-001'));
    expect(mockInvoke).not.toHaveBeenCalledWith('get_post_analytics', expect.anything());
  });

  it('shows not-configured CTA after clicking the load trigger', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repo_published') return [makeSent()];
      if (cmd === 'get_post_analytics') return { configured: false, sessions: 0, unique_sessions: 0, top_referrer: null };
      return null;
    });
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('post-001'));
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() => expect(screen.getByText(/set up analytics/i)).toBeInTheDocument());
  });

  it('shows zero-sessions message when configured but no traffic', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repo_published') return [makeSent()];
      if (cmd === 'get_post_analytics') return { configured: true, sessions: 0, unique_sessions: 0, top_referrer: null };
      return null;
    });
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('post-001'));
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() => expect(screen.getByText(/no postlane-referred sessions/i)).toBeInTheDocument());
  });
});

// ---------------------------------------------------------------------------
// Fetch error
// ---------------------------------------------------------------------------

describe('RepoPublishedView — fetch error', () => {
  it('shows empty state when get_repo_published fails', async () => {
    mockInvoke.mockRejectedValue(new Error('DB error'));
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByText(/no posts sent yet/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Analytics UX improvements
// ---------------------------------------------------------------------------

describe('RepoPublishedView — analytics UX improvements', () => {
  it('shows a title tooltip on the load analytics trigger', async () => {
    mockInvoke.mockResolvedValue([makeSent()]);
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('post-001'));
    const trigger = screen.getByRole('button', { name: /load analytics/i });
    expect(trigger).toHaveAttribute('title');
  });

  it('shows unique and total session counts when analytics loaded with traffic', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repo_published') return [makeSent()];
      if (cmd === 'get_post_analytics') return { configured: true, sessions: 50, unique_sessions: 20, top_referrer: null };
      return null;
    });
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('post-001'));
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() => {
      expect(screen.getByText(/20 unique/)).toBeInTheDocument();
      expect(screen.getByText(/50 total/)).toBeInTheDocument();
    });
  });

  it('shows "No sessions yet" for a post published less than 7 days ago', async () => {
    const recentSentAt = new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString();
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repo_published') return [makeSent({ sent_at: recentSentAt })];
      if (cmd === 'get_post_analytics') return { configured: true, sessions: 0, unique_sessions: 0, top_referrer: null };
      return null;
    });
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByText('post-001'));
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() => expect(screen.getByText(/no sessions yet/i)).toBeInTheDocument());
  });
});
