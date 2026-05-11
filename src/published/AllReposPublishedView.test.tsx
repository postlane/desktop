// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import AllReposPublishedView from './AllReposPublishedView';
import type { PublishedPost } from '../types';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));

import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

function makeSent(overrides: Partial<PublishedPost> = {}): PublishedPost {
  return {
    repo_id: 'r1',
    repo_name: 'my-app',
    repo_path: '/path/to/repo',
    post_folder: 'post-001',
    status: 'sent',
    platforms: ['x'],
    platform_results: { x: 'sent' },
    schedule: null,
    scheduler_ids: null,
    platform_urls: null,
    provider: null,
    llm_model: 'claude-sonnet-4-6',
    sent_at: '2026-04-15T10:00:00Z',
    created_at: '2026-04-15T09:00:00Z',
    ...overrides,
  };
}

function setupMocks(posts: PublishedPost[]) {
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'get_all_published') return posts;
    if (cmd === 'export_history_csv') return '/Users/test/Downloads/postlane-history.csv';
    if (cmd === 'get_post_analytics') return { sessions: 0, unique_sessions: 0, top_referrer: null };
    return null;
  });
}

// ---------------------------------------------------------------------------
// Empty state
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — empty state', () => {
  it('shows empty state when no posts', async () => {
    setupMocks([]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/no posts published yet/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Sent posts table
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — sent posts table', () => {
  it('shows Repo column with repo name', async () => {
    setupMocks([makeSent({ repo_name: 'my-app' })]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getAllByText('my-app').length).toBeGreaterThan(0),
    );
  });

  it('clicking repo badge calls onNavigateToRepo', async () => {
    const onNav = vi.fn();
    setupMocks([makeSent({ repo_id: 'r1', repo_name: 'my-app' })]);
    render(<AllReposPublishedView onNavigateToRepo={onNav} />);
    await waitFor(() => screen.getAllByText('my-app'));
    fireEvent.click(screen.getAllByText('my-app')[0]);
    expect(onNav).toHaveBeenCalledWith('r1');
  });

  it('shows Load more when >100 posts', async () => {
    const posts = Array.from({ length: 101 }, (_, i) =>
      makeSent({ post_folder: `p${String(i).padStart(3, '0')}` }),
    );
    setupMocks(posts);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(
      () => expect(screen.getByRole('button', { name: /load more/i })).toBeInTheDocument(),
      { timeout: 10000 },
    );
  }, 12000);

  it('shows Load more when 101 raw posts but some fail the isPublishedPost guard', async () => {
    // 100 valid sent posts + 1 invalid-shape post = 101 raw items
    // hasMore must be based on raw count (101 > 100) not filtered count
    const validPosts = Array.from({ length: 100 }, (_, i) =>
      makeSent({ post_folder: `p${String(i).padStart(3, '0')}` }),
    );
    const badShapePost = { repo_id: 'r1', post_folder: 'bad', status: 'sent', platforms: 'not-an-array' };
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_published') return [...validPosts, badShapePost];
      return null;
    });
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(
      () => expect(screen.getByRole('button', { name: /load more/i })).toBeInTheDocument(),
      { timeout: 10000 },
    );
  }, 12000);
});

// ---------------------------------------------------------------------------
// Export CSV
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — export CSV', () => {
  it('shows Export CSV button', async () => {
    setupMocks([makeSent()]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /export csv/i })).toBeInTheDocument(),
    );
  });

  it('calls export_history_csv and shows success path', async () => {
    setupMocks([makeSent()]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /export csv/i }));
    fireEvent.click(screen.getByRole('button', { name: /export csv/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('export_history_csv'),
    );
  });
});

// ---------------------------------------------------------------------------
// Scheduler column
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — scheduler column', () => {
  it('shows provider name in Scheduler column when present', async () => {
    setupMocks([makeSent({ provider: 'zernio' })]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    expect(screen.getByText('zernio')).toBeInTheDocument();
  });

  it('shows — in Scheduler column when provider is null', async () => {
    setupMocks([makeSent({ provider: null })]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    expect(screen.getAllByText('—').length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// View links
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — view links', () => {
  it('shows — when platform_urls is null', async () => {
    setupMocks([makeSent({ platform_urls: null })]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    expect(screen.getAllByText('—').length).toBeGreaterThan(0);
  });

  it('shows clickable view link when platform_urls has a URL', async () => {
    setupMocks([makeSent({
      platform_results: { x: 'sent' },
      platform_urls: { x: 'https://x.com/i/web/status/42' },
    })]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    expect(screen.getByRole('button', { name: /view x post/i })).toBeInTheDocument();
  });

  it('clicking view link invokes opener with the correct URL', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_published')
        return [makeSent({
          platform_results: { x: 'sent' },
          platform_urls: { x: 'https://x.com/i/web/status/77' },
        })];
      return undefined;
    });
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /view x post/i }));
    fireEvent.click(screen.getByRole('button', { name: /view x post/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('plugin:opener|open_url', {
        url: 'https://x.com/i/web/status/77',
      }),
    );
  });
});

// ---------------------------------------------------------------------------
// Renders correctly
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — renders correctly', () => {
  it('renders without crashing', async () => {
    setupMocks([]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/no posts published yet/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Export CSV — error path
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — export error', () => {
  it('shows error message when export fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_published') return [makeSent()];
      if (cmd === 'export_history_csv') throw new Error('Permission denied');
      return null;
    });
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /export csv/i }));
    fireEvent.click(screen.getByRole('button', { name: /export csv/i }));
    await waitFor(() =>
      expect(screen.getByText(/permission denied/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Load more
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — load more', () => {
  it('clicking Load more appends next page', async () => {
    const firstPage = Array.from({ length: 101 }, (_, i) =>
      makeSent({ post_folder: `p${String(i).padStart(3, '0')}` }),
    );
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_published') return firstPage;
      if (cmd === 'get_post_analytics') return { sessions: 0, unique_sessions: 0, top_referrer: null };
      return null;
    });
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /load more/i }), { timeout: 10000 });
    fireEvent.click(screen.getByRole('button', { name: /load more/i }));
    await waitFor(
      () => expect(mockInvoke).toHaveBeenCalledWith('get_all_published', expect.objectContaining({ offset: 100 })),
      { timeout: 10000 },
    );
  }, 15000);
});

// ---------------------------------------------------------------------------
// Analytics lazy load + UX states
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — analytics lazy load', () => {
  it('does not call get_post_analytics on initial render', async () => {
    setupMocks([makeSent()]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    expect(mockInvoke).not.toHaveBeenCalledWith('get_post_analytics', expect.anything());
  });

  it('shows not-configured CTA after clicking the load trigger', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_published') return [makeSent()];
      if (cmd === 'get_post_analytics') return { configured: false, sessions: 0, unique_sessions: 0, top_referrer: null };
      return null;
    });
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() => expect(screen.getByText(/set up analytics/i)).toBeInTheDocument());
  });

  it('shows zero-sessions message when configured but no traffic', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_published') return [makeSent()];
      if (cmd === 'get_post_analytics') return { configured: true, sessions: 0, unique_sessions: 0, top_referrer: null };
      return null;
    });
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() => expect(screen.getByText(/no postlane-referred sessions/i)).toBeInTheDocument());
  });
});

// ---------------------------------------------------------------------------
// Fetch error
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — fetch error', () => {
  it('shows empty state when get_all_published fails', async () => {
    mockInvoke.mockRejectedValue(new Error('network error'));
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/no posts published yet/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Analytics UX improvements
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — analytics UX improvements', () => {
  it('shows a title tooltip on the load analytics trigger', async () => {
    setupMocks([makeSent()]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    const trigger = screen.getByRole('button', { name: /load analytics/i });
    expect(trigger).toHaveAttribute('title');
  });

  it('shows unique and total session counts when analytics are loaded with traffic', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_published') return [makeSent()];
      if (cmd === 'get_post_analytics') return { configured: true, sessions: 100, unique_sessions: 42, top_referrer: null };
      return null;
    });
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() => {
      expect(screen.getByText(/42 unique/)).toBeInTheDocument();
      expect(screen.getByText(/100 total/)).toBeInTheDocument();
    });
  });

  it('shows "No sessions yet" for a post published less than 7 days ago with zero sessions', async () => {
    const recentSentAt = new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString();
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_published') return [makeSent({ sent_at: recentSentAt })];
      if (cmd === 'get_post_analytics') return { configured: true, sessions: 0, unique_sessions: 0, top_referrer: null };
      return null;
    });
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() => expect(screen.getByText(/no sessions yet/i)).toBeInTheDocument());
  });
});

// ---------------------------------------------------------------------------
// Link open error
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — link open error', () => {
  it('shows error when opener fails (§review-silentcatch)', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_published')
        return [makeSent({ platform_results: { x: 'sent' }, platform_urls: { x: 'https://x.com/i/web/status/77' } })];
      if (cmd === 'plugin:opener|open_url') throw new Error('Opener failed');
      return null;
    });
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /view x post/i }));
    fireEvent.click(screen.getByRole('button', { name: /view x post/i }));
    await waitFor(() => expect(screen.getByText(/opener failed/i)).toBeInTheDocument());
  });
});

// IPC guard
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — IPC guard', () => {
  it('test_filters_invalid_ipc_shapes', async () => {
    const badShape = { repo_id: 'r1', post_folder: 'bad-post', status: 'sent', sent_at: null, platforms: 'not-an-array' };
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_published') return [makeSent({ post_folder: 'valid-post' }), badShape];
      return null;
    });
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('valid-post'));
    expect(screen.queryByText('bad-post')).not.toBeInTheDocument();
  });
});
