// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import AllReposDraftsView from './AllReposDraftsView';
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

// ---------------------------------------------------------------------------
// Loading and empty state
// ---------------------------------------------------------------------------

describe('AllReposDraftsView — empty state', () => {
  it('shows empty state when no drafts exist', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/no drafts waiting/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Grouping
// ---------------------------------------------------------------------------

describe('AllReposDraftsView — grouping', () => {
  it('shows repo group headers', async () => {
    mockInvoke.mockResolvedValue([
      makePost({ repo_id: 'r1', repo_name: 'app-one' }),
      makePost({ repo_id: 'r2', repo_name: 'app-two', post_folder: 'post-002' }),
    ]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => {
      // Group headers are <h2> elements
      expect(screen.getAllByText('app-one').length).toBeGreaterThan(0);
      expect(screen.getAllByText('app-two').length).toBeGreaterThan(0);
    });
  });

  it('renders failed posts before ready posts within a group', async () => {
    mockInvoke.mockResolvedValue([
      makePost({ status: 'ready', post_folder: 'p1', trigger: 'Ready post', created_at: '2026-04-15T10:00:00Z' }),
      makePost({ status: 'failed', post_folder: 'p2', trigger: 'Failed post', created_at: '2026-04-15T09:00:00Z', error: 'err' }),
    ]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => {
      const cards = screen.getAllByText(/post/i);
      expect(cards[0].textContent).toContain('Failed');
    });
  });
});

// ---------------------------------------------------------------------------
// Approve all ready
// ---------------------------------------------------------------------------

describe('AllReposDraftsView — approve all ready', () => {
  it('shows "Approve all ready" button when 2+ ready posts exist', async () => {
    mockInvoke.mockResolvedValue([
      makePost({ post_folder: 'p1', trigger: 'Post 1' }),
      makePost({ post_folder: 'p2', trigger: 'Post 2' }),
    ]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /approve all ready/i })).toBeInTheDocument(),
    );
  });

  it('does not show "Approve all ready" with only 1 ready post', async () => {
    mockInvoke.mockResolvedValue([makePost()]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByText('Test post'));
    expect(screen.queryByRole('button', { name: /approve all ready/i })).not.toBeInTheDocument();
  });

  it('shows confirmation dialog before approving all', async () => {
    mockInvoke.mockResolvedValue([
      makePost({ post_folder: 'p1', trigger: 'Post 1' }),
      makePost({ post_folder: 'p2', trigger: 'Post 2' }),
    ]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() =>
      expect(screen.getByRole('dialog')).toBeInTheDocument(),
    );
  });

  it('approves all posts in sequence on confirm', async () => {
    const drafts = [
      makePost({ post_folder: 'p1', trigger: 'Post 1' }),
      makePost({ post_folder: 'p2', trigger: 'Post 2' }),
    ];
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_drafts') return drafts;
      if (cmd === 'approve_post') return { success: true };
      return null;
    });

    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() => screen.getByRole('dialog'));
    fireEvent.click(screen.getByRole('button', { name: /^confirm$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('approve_post', expect.objectContaining({ postFolder: 'p1' })),
    );
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('approve_post', expect.objectContaining({ postFolder: 'p2' })),
    );
  });
});

// ---------------------------------------------------------------------------
// Cmd+Enter shortcut
// ---------------------------------------------------------------------------

describe('AllReposDraftsView — Cmd+Enter shortcut', () => {
  it('Cmd+Enter opens approve-all dialog when 2+ ready posts', async () => {
    mockInvoke.mockResolvedValue([
      makePost({ post_folder: 'p1', trigger: 'Post 1' }),
      makePost({ post_folder: 'p2', trigger: 'Post 2' }),
    ]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.keyDown(document, { key: 'Enter', metaKey: true });
    await waitFor(() => expect(screen.getByRole('dialog')).toBeInTheDocument());
  });
});
