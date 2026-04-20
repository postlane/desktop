// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import AllReposPublishedView from './AllReposPublishedView';
import type { PublishedPost, ModelStats } from '../types';

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

function makeStats(overrides: Partial<ModelStats> = {}): ModelStats {
  return {
    model: 'claude-sonnet-4-6',
    total_posts: 20,
    edited_posts: 5,
    edit_rate: 0.25,
    limited_data: false,
    ...overrides,
  };
}

function setupMocks(posts: PublishedPost[], stats: ModelStats[]) {
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'get_all_published') return posts;
    if (cmd === 'get_model_stats') return stats;
    if (cmd === 'export_history_csv') return '/Users/test/Downloads/postlane-history.csv';
    return null;
  });
}

// ---------------------------------------------------------------------------
// Empty state
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — empty state', () => {
  it('shows empty state when no posts', async () => {
    setupMocks([], []);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/no posts published yet/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Model comparison bar
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — model comparison bar', () => {
  it('hidden when fewer than 10 total sent posts', async () => {
    const posts = Array.from({ length: 9 }, (_, i) =>
      makeSent({ post_folder: `p${i}` }),
    );
    setupMocks(posts, [makeStats({ total_posts: 9 })]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('p0'));
    expect(screen.queryByText(/edit rate/i)).not.toBeInTheDocument();
  });

  it('shown when 10+ total sent posts', async () => {
    const posts = Array.from({ length: 10 }, (_, i) =>
      makeSent({ post_folder: `p${i}` }),
    );
    setupMocks(posts, [makeStats({ total_posts: 10 })]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/edit rate/i)).toBeInTheDocument(),
    );
  });

  it('shows model name and edit rate percentage', async () => {
    const posts = Array.from({ length: 10 }, (_, i) => makeSent({ post_folder: `p${i}` }));
    setupMocks(posts, [makeStats({ model: 'claude-sonnet-4-6', edit_rate: 0.25, total_posts: 20 })]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText(/edit rate/i));
    expect(screen.getAllByText(/claude-sonnet-4-6/).length).toBeGreaterThan(0);
    expect(screen.getByText(/25%/)).toBeInTheDocument();
  });

  it('shows "Limited data" label for models with 5–19 posts', async () => {
    const posts = Array.from({ length: 10 }, (_, i) => makeSent({ post_folder: `p${i}` }));
    setupMocks(posts, [makeStats({ total_posts: 10, limited_data: true })]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText(/edit rate/i));
    expect(screen.getByText(/limited data/i)).toBeInTheDocument();
  });

  it('shows edit rate tooltip text', async () => {
    const posts = Array.from({ length: 10 }, (_, i) => makeSent({ post_folder: `p${i}` }));
    setupMocks(posts, [makeStats({ total_posts: 10 })]);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText(/edit rate/i));
    expect(screen.getByTitle(/how often you changed/i)).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Sent posts table
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — sent posts table', () => {
  it('shows Repo column with repo name', async () => {
    setupMocks([makeSent({ repo_name: 'my-app' })], []);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getAllByText('my-app').length).toBeGreaterThan(0),
    );
  });

  it('clicking repo badge calls onNavigateToRepo', async () => {
    const onNav = vi.fn();
    setupMocks([makeSent({ repo_id: 'r1', repo_name: 'my-app' })], []);
    render(<AllReposPublishedView onNavigateToRepo={onNav} />);
    await waitFor(() => screen.getAllByText('my-app'));
    fireEvent.click(screen.getAllByText('my-app')[0]);
    expect(onNav).toHaveBeenCalledWith('r1');
  });

  it('shows Load more when >100 posts', async () => {
    const posts = Array.from({ length: 101 }, (_, i) =>
      makeSent({ post_folder: `p${String(i).padStart(3, '0')}` }),
    );
    setupMocks(posts, []);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /load more/i })).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Export CSV
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — export CSV', () => {
  it('shows Export CSV button', async () => {
    setupMocks([makeSent()], []);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /export csv/i })).toBeInTheDocument(),
    );
  });

  it('calls export_history_csv and shows success path', async () => {
    setupMocks([makeSent()], []);
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
    setupMocks([makeSent({ provider: 'zernio' })], []);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    expect(screen.getByText('zernio')).toBeInTheDocument();
  });

  it('shows — in Scheduler column when provider is null', async () => {
    setupMocks([makeSent({ provider: null })], []);
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
    setupMocks([makeSent({ platform_urls: null })], []);
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByText('post-001'));
    expect(screen.getAllByText('—').length).toBeGreaterThan(0);
  });

  it('shows clickable view link when platform_urls has a URL', async () => {
    setupMocks(
      [makeSent({
        platform_results: { x: 'sent' },
        platform_urls: { x: 'https://x.com/i/web/status/42' },
      })],
      [],
    );
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
      if (cmd === 'get_model_stats') return [];
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
// Cmd+H keyboard shortcut (tested at App level — just verify the component renders)
// ---------------------------------------------------------------------------

describe('AllReposPublishedView — renders correctly', () => {
  it('renders without crashing', async () => {
    setupMocks([], []);
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
      if (cmd === 'get_model_stats') return [];
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
      if (cmd === 'get_model_stats') return [];
      return null;
    });
    render(<AllReposPublishedView onNavigateToRepo={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /load more/i }));
    fireEvent.click(screen.getByRole('button', { name: /load more/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(3)); // initial: 2 calls, load more: 1 more
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
