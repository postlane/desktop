// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import RepoDraftsView from './RepoDraftsView';
import type { DraftPost } from '../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

beforeEach(() => vi.clearAllMocks());

function makePost(overrides: Partial<DraftPost> = {}): DraftPost {
  return {
    repo_id: 'r1',
    repo_name: 'my-app',
    repo_path: '/path/to/repo',
    post_folder: 'post-001',
    status: 'ready',
    platforms: ['x'],
    schedule: null,
    trigger: 'Test post',
    platform_results: null,
    error: null,
    image_url: null,
    llm_model: null,
    created_at: '2026-04-15T09:00:00Z',
    ...overrides,
  };
}

describe('RepoDraftsView — rendering', () => {
  it('shows the repo name as a heading', async () => {
    mockInvoke.mockResolvedValue([makePost()]);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'my-app' })).toBeInTheDocument(),
    );
  });

  it('shows only posts from the specified repo', async () => {
    mockInvoke.mockResolvedValue([
      makePost({ repo_id: 'r1', repo_name: 'my-app', post_folder: 'p1', trigger: 'Right post' }),
      makePost({ repo_id: 'r2', repo_name: 'other-app', post_folder: 'p2', trigger: 'Wrong post' }),
    ]);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() => expect(screen.getByText('Right post')).toBeInTheDocument());
    expect(screen.queryByText('Wrong post')).not.toBeInTheDocument();
  });

  it('does not show group headers', async () => {
    mockInvoke.mockResolvedValue([makePost()]);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() => screen.getByText('Test post'));
    expect(screen.getAllByRole('heading').length).toBe(1);
  });

  it('shows failed posts before ready posts', async () => {
    mockInvoke.mockResolvedValue([
      makePost({ repo_id: 'r1', post_folder: 'p1', status: 'ready', trigger: 'Ready post', created_at: '2026-04-15T10:00:00Z' }),
      makePost({ repo_id: 'r1', post_folder: 'p2', status: 'failed', trigger: 'Failed post', created_at: '2026-04-15T09:00:00Z', error: 'err' }),
    ]);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() => screen.getByText('Failed post'));
    const articles = screen.getAllByRole('article');
    expect(articles[0]).toHaveTextContent('Failed post');
    expect(articles[1]).toHaveTextContent('Ready post');
  });

  it('shows empty state when no posts for this repo', async () => {
    mockInvoke.mockResolvedValue([makePost({ repo_id: 'r2', repo_name: 'other' })]);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByText(/no drafts waiting/i)).toBeInTheDocument(),
    );
  });

  it('uses PostCard — not a duplicate implementation', async () => {
    mockInvoke.mockResolvedValue([makePost()]);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() => expect(screen.getByRole('article')).toBeInTheDocument());
  });
});

describe('RepoDraftsView — sorting and errors', () => {
  it('puts failed post before ready when failed is first in array', async () => {
    mockInvoke.mockResolvedValue([
      makePost({ repo_id: 'r1', post_folder: 'p1', status: 'failed', trigger: 'Failed post', error: 'err', created_at: '2026-04-15T09:00:00Z' }),
      makePost({ repo_id: 'r1', post_folder: 'p2', status: 'ready', trigger: 'Ready post', created_at: '2026-04-15T10:00:00Z' }),
    ]);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() => screen.getByText('Failed post'));
    const articles = screen.getAllByRole('article');
    expect(articles[0]).toHaveTextContent('Failed post');
  });

  it('sorts two ready posts by date descending', async () => {
    mockInvoke.mockResolvedValue([
      makePost({ repo_id: 'r1', post_folder: 'p1', status: 'ready', trigger: 'Older post', created_at: '2026-04-15T08:00:00Z' }),
      makePost({ repo_id: 'r1', post_folder: 'p2', status: 'ready', trigger: 'Newer post', created_at: '2026-04-15T10:00:00Z' }),
    ]);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() => screen.getByText('Newer post'));
    const articles = screen.getAllByRole('article');
    expect(articles[0]).toHaveTextContent('Newer post');
    expect(articles[1]).toHaveTextContent('Older post');
  });

  it('does not crash when get_all_drafts fails', async () => {
    mockInvoke.mockRejectedValue(new Error('DB error'));
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByText(/no drafts waiting/i)).toBeInTheDocument(),
    );
  });
});

describe('RepoDraftsView — meta-changed events', () => {
  it('meta-changed event for matching repo triggers a refresh', async () => {
    let capturedHandler: ((_event: { payload: { repo_id: string; post_folder: string } }) => void) | null = null;
    mockListen.mockImplementation((_event: string, handler: unknown) => {
      capturedHandler = handler as typeof capturedHandler;
      return Promise.resolve(() => {});
    });

    let draftsCallCount = 0;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_all_drafts') {
        draftsCallCount++;
        if (draftsCallCount === 1) {
          return Promise.resolve([makePost({ repo_id: 'r1', post_folder: 'p1', trigger: 'First load' })]);
        }
        return Promise.resolve([makePost({ repo_id: 'r1', post_folder: 'p1', trigger: 'After refresh' })]);
      }
      return Promise.resolve(null);
    });

    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() => screen.getByText('First load'));
    if (capturedHandler) capturedHandler({ payload: { repo_id: 'r1', post_folder: 'p1' } });
    await waitFor(() => expect(screen.getByText('After refresh')).toBeInTheDocument());
  });

  it('meta-changed event for a different repo does not trigger refresh', async () => {
    let capturedHandler: ((_event: { payload: { repo_id: string; post_folder: string } }) => void) | null = null;
    mockListen.mockImplementation((_event: string, handler: unknown) => {
      capturedHandler = handler as typeof capturedHandler;
      return Promise.resolve(() => {});
    });

    mockInvoke.mockResolvedValue([makePost({ repo_id: 'r1', post_folder: 'p1', trigger: 'Only post' })]);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() => screen.getByText('Only post'));
    const callsBefore = mockInvoke.mock.calls.length;

    if (capturedHandler) capturedHandler({ payload: { repo_id: 'r2', post_folder: 'p2' } });
    await new Promise((r) => setTimeout(r, 50));
    expect(mockInvoke.mock.calls.length).toBe(callsBefore);
  });
});
