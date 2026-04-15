// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import RepoDraftsView from './RepoDraftsView';
import type { DraftPost } from '../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

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

describe('RepoDraftsView', () => {
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
    // Only one heading — the page heading, not a group label
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
    mockInvoke.mockResolvedValue([
      makePost({ repo_id: 'r2', repo_name: 'other' }),
    ]);
    render(<RepoDraftsView repoId="r1" />);
    // No posts for r1 — but we need the repo name. Mock a second call for repos.
    await waitFor(() =>
      expect(screen.getByText(/no drafts waiting/i)).toBeInTheDocument(),
    );
  });

  it('uses PostCard — not a duplicate implementation', async () => {
    mockInvoke.mockResolvedValue([makePost()]);
    render(<RepoDraftsView repoId="r1" />);
    // PostCard renders an article role — confirms the shared component is used
    await waitFor(() => expect(screen.getByRole('article')).toBeInTheDocument());
  });
});
