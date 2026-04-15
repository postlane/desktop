// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import PostCard from './PostCard';
import type { DraftPost } from '../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makePost(overrides: Partial<DraftPost> = {}): DraftPost {
  return {
    repo_id: 'r1',
    repo_name: 'my-app',
    repo_path: '/path/to/repo',
    post_folder: 'post-001',
    status: 'ready',
    platforms: ['x', 'bluesky'],
    schedule: '2026-06-01T10:00:00Z',
    trigger: 'Launched v2.0',
    platform_results: null,
    error: null,
    image_url: null,
    llm_model: 'claude-sonnet-4-6',
    created_at: '2026-04-15T09:00:00Z',
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Collapsed state
// ---------------------------------------------------------------------------

describe('PostCard — collapsed state', () => {
  it('shows the trigger text', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText('Launched v2.0')).toBeInTheDocument();
  });

  it('shows repo badge', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText('my-app')).toBeInTheDocument();
  });

  it('shows platforms', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText(/x/i)).toBeInTheDocument();
  });

  it('shows Approve button', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByRole('button', { name: /approve/i })).toBeInTheDocument();
  });

  it('shows Preview toggle button', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByRole('button', { name: /preview/i })).toBeInTheDocument();
  });

  it('shows dismiss button', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByRole('button', { name: /dismiss/i })).toBeInTheDocument();
  });

  it('falls back to first 80 chars of post_folder slug when trigger is null', () => {
    const post = makePost({ trigger: null, post_folder: 'my-interesting-post-about-things' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText('my-interesting-post-about-things')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Expanded state
// ---------------------------------------------------------------------------

describe('PostCard — expanded state', () => {
  it('clicking Preview expands the card', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    expect(screen.getByRole('tablist')).toBeInTheDocument();
  });

  it('shows platform tabs when expanded', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    expect(screen.getByRole('tab', { name: /x/i })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /bluesky/i })).toBeInTheDocument();
  });

  it('clicking Preview again collapses the card', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Failed state
// ---------------------------------------------------------------------------

describe('PostCard — failed state', () => {
  it('always starts expanded', () => {
    const post = makePost({ status: 'failed', error: 'Scheduler timeout' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByRole('tablist')).toBeInTheDocument();
  });

  it('shows FAILED badge', () => {
    const post = makePost({ status: 'failed', error: 'Scheduler timeout' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText(/failed/i)).toBeInTheDocument();
  });

  it('shows error message', () => {
    const post = makePost({ status: 'failed', error: 'Scheduler timeout' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText('Scheduler timeout')).toBeInTheDocument();
  });

  it('shows Retry button', () => {
    const post = makePost({ status: 'failed', error: 'Scheduler timeout' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
  });

  it('shows per-platform results', () => {
    const post = makePost({
      status: 'failed',
      error: 'Partial failure',
      platform_results: { x: 'sent', bluesky: 'failed' },
      platforms: [], // no tabs, so "bluesky" only appears in results
    });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getAllByText(/bluesky/i).length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// Approve action
// ---------------------------------------------------------------------------

describe('PostCard — approve', () => {
  it('calls approve_post and fires onApproved', async () => {
    const onApproved = vi.fn();
    mockInvoke.mockResolvedValue({ success: true, platform_results: null, error: null });
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce());
    expect(mockInvoke).toHaveBeenCalledWith('approve_post', expect.objectContaining({
      repoPath: '/path/to/repo',
      postFolder: 'post-001',
    }));
  });

  it('shows error inline on approval failure without crashing', async () => {
    mockInvoke.mockRejectedValue(new Error('Network error'));
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    await waitFor(() => expect(screen.getByText(/network error/i)).toBeInTheDocument());
  });
});

// ---------------------------------------------------------------------------
// Dismiss action
// ---------------------------------------------------------------------------

describe('PostCard — dismiss', () => {
  it('calls dismiss_post and fires onDismissed', async () => {
    const onDismissed = vi.fn();
    mockInvoke.mockResolvedValue(undefined);
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={onDismissed} />);
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    await waitFor(() => expect(onDismissed).toHaveBeenCalledOnce());
    expect(mockInvoke).toHaveBeenCalledWith('dismiss_post', expect.objectContaining({
      repoPath: '/path/to/repo',
      postFolder: 'post-001',
    }));
  });
});

// ---------------------------------------------------------------------------
// Keyboard shortcuts
// ---------------------------------------------------------------------------

describe('PostCard — keyboard shortcuts', () => {
  it('A key approves the focused card', async () => {
    const onApproved = vi.fn();
    mockInvoke.mockResolvedValue({ success: true });
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} isFocused />);
    const card = screen.getByRole('article');
    fireEvent.keyDown(card, { key: 'a' });
    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce());
  });

  it('D key dismisses the focused card', async () => {
    const onDismissed = vi.fn();
    mockInvoke.mockResolvedValue(undefined);
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={onDismissed} isFocused />);
    const card = screen.getByRole('article');
    fireEvent.keyDown(card, { key: 'd' });
    await waitFor(() => expect(onDismissed).toHaveBeenCalledOnce());
  });

  it('E key toggles expanded state', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} isFocused />);
    const card = screen.getByRole('article');
    fireEvent.keyDown(card, { key: 'e' });
    expect(screen.getByRole('tablist')).toBeInTheDocument();
    fireEvent.keyDown(card, { key: 'e' });
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
  });

  it('Escape collapses an expanded card', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} isFocused />);
    const card = screen.getByRole('article');
    fireEvent.keyDown(card, { key: 'e' });
    expect(screen.getByRole('tablist')).toBeInTheDocument();
    fireEvent.keyDown(card, { key: 'Escape' });
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
  });
});
