// SPDX-License-Identifier: BUSL-1.1
// Dialog, billing-gate, and platform-argument tests for AllReposDraftsView.
// Kept separate to keep each file under the 400-line limit.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import AllReposDraftsView from './AllReposDraftsView';
import type { DraftPost } from '../types';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

function makePost(overrides: Partial<DraftPost> = {}): DraftPost {
  return {
    repo_id: 'r1', repo_name: 'my-app', repo_path: '/path/to/repo',
    post_folder: 'post-001', status: 'ready', platforms: ['x'],
    schedule: null, trigger: 'Test post', platform_results: null, error: null,
    image_url: null, llm_model: null, created_at: '2026-04-15T09:00:00Z',
    project_id: null, platform: 'x', text: '',
    ...overrides,
  };
}

// ── ApproveAllDialog — result icons ──────────────────────────────────────────

function makePausedApprove(failFirst = false) {
  const resolver = { resolve: (_v: { success: boolean }) => {} };
  const secondPromise = new Promise<{ success: boolean }>((res) => { resolver.resolve = res; });
  const drafts = [makePost({ post_folder: 'p1', trigger: 'Post 1' }), makePost({ post_folder: 'p2', trigger: 'Post 2' })];
  let approveCallCount = 0;
  const handler = async (cmd: unknown) => {
    if (cmd === 'get_all_drafts') return drafts;
    if (cmd === 'approve_post') {
      approveCallCount++;
      if (approveCallCount === 1) { if (failFirst) throw new Error('fail'); return { success: true }; }
      return secondPromise;
    }
    return null;
  };
  return { resolver, handler, drafts };
}

describe('AllReposDraftsView — approve all dialog result icons', () => {
  it('shows success icon while second approval is pending', async () => {
    const { resolver, handler } = makePausedApprove(false);
    mockInvoke.mockImplementation(handler);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() => screen.getByRole('dialog'));
    fireEvent.click(screen.getByRole('button', { name: /^confirm$/i }));
    await waitFor(() => expect(screen.getByText('✓')).toBeInTheDocument());
    resolver.resolve({ success: true });
  });

  it('shows error icon while second approval is pending after first fails', async () => {
    const { resolver, handler } = makePausedApprove(true);
    mockInvoke.mockImplementation(handler);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() => screen.getByRole('dialog'));
    fireEvent.click(screen.getByRole('button', { name: /^confirm$/i }));
    await waitFor(() => expect(screen.getByText('✗')).toBeInTheDocument());
    resolver.resolve({ success: true });
  });
});

// ── ApproveAllDialog — running state and dismiss ──────────────────────────────

function makeStalledApprove() {
  const resolver = { resolve: (_v: { success: boolean }) => {} };
  const approvePromise = new Promise<{ success: boolean }>((res) => { resolver.resolve = res; });
  const drafts = [makePost({ post_folder: 'p1', trigger: 'Post 1' }), makePost({ post_folder: 'p2', trigger: 'Post 2' })];
  const handler = async (cmd: unknown) => {
    if (cmd === 'get_all_drafts') return drafts;
    if (cmd === 'approve_post') return approvePromise;
    return null;
  };
  return { resolver, handler };
}

describe('AllReposDraftsView — approve all dialog dismiss and running', () => {
  it('shows plural "posts" in dialog body when readyCount is 2', async () => {
    mockInvoke.mockResolvedValue([makePost({ post_folder: 'p1' }), makePost({ post_folder: 'p2' })]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() => screen.getByRole('dialog'));
    expect(screen.getByText(/send 2 posts to your scheduler/i)).toBeInTheDocument();
  });

  it('shows Sending… label while approve is running', async () => {
    const { resolver, handler } = makeStalledApprove();
    mockInvoke.mockImplementation(handler);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() => screen.getByRole('dialog'));
    fireEvent.click(screen.getByRole('button', { name: /^confirm$/i }));
    await waitFor(() => expect(screen.getByText(/sending/i)).toBeInTheDocument());
    resolver.resolve({ success: true });
  });

  it('closes dialog when the × button is clicked', async () => {
    mockInvoke.mockResolvedValue([makePost({ post_folder: 'p1' }), makePost({ post_folder: 'p2' })]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() => screen.getByRole('dialog'));
    fireEvent.click(screen.getByRole('button', { name: /^close$/i }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('closes dialog when modal background is clicked', async () => {
    mockInvoke.mockResolvedValue([makePost({ post_folder: 'p1' }), makePost({ post_folder: 'p2' })]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() => screen.getByRole('dialog'));
    const bg = document.querySelector('.modal-background');
    if (bg) fireEvent.click(bg);
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });
});

// ── Ctrl+Enter shortcut (non-Mac) ─────────────────────────────────────────────

describe('AllReposDraftsView — Ctrl+Enter shortcut', () => {
  it('Ctrl+Enter opens approve-all dialog when 2+ ready posts', async () => {
    mockInvoke.mockResolvedValue([makePost({ post_folder: 'p1' }), makePost({ post_folder: 'p2' })]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.keyDown(document, { key: 'Enter', ctrlKey: true });
    await waitFor(() => expect(screen.getByRole('dialog')).toBeInTheDocument());
  });
});

// ── FIN-C2: billing gate on Approve All ──────────────────────────────────────

describe('AllReposDraftsView — billing gate on approve all (FIN-C2)', () => {
  it('does not call approve_post when billingActive is false', async () => {
    const drafts = [makePost({ post_folder: 'p1', trigger: 'Post 1' }), makePost({ post_folder: 'p2', trigger: 'Post 2' })];
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_drafts') return drafts;
      return null;
    });
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} billingActive={false} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() => screen.getByRole('dialog'));
    fireEvent.click(screen.getByRole('button', { name: /^confirm$/i }));
    await waitFor(() => {
      const approveCalls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'approve_post');
      expect(approveCalls).toHaveLength(0);
    });
  });

  it('shows billing-inactive message in dialog when billingActive is false', async () => {
    mockInvoke.mockResolvedValue([makePost({ post_folder: 'p1' }), makePost({ post_folder: 'p2' })]);
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} billingActive={false} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() => screen.getByRole('dialog'));
    expect(screen.getByText(/billing/i)).toBeInTheDocument();
  });

  it('calls approve_post normally when billingActive is true', async () => {
    const drafts = [makePost({ post_folder: 'p1' }), makePost({ post_folder: 'p2' })];
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_drafts') return drafts;
      if (cmd === 'approve_post') return null;
      return null;
    });
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} billingActive={true} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() => screen.getByRole('dialog'));
    fireEvent.click(screen.getByRole('button', { name: /^confirm$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('approve_post', expect.objectContaining({ postFolder: 'p1' })),
    );
  });
});

// ── approve-all passes platform argument ─────────────────────────────────────

describe('AllReposDraftsView — approve-all passes platform argument (CRITICAL-1)', () => {
  it('approve_post invoke includes the platform field for each post', async () => {
    const drafts = [makePost({ post_folder: 'p1', platform: 'x' }), makePost({ post_folder: 'p2', platform: 'bluesky' })];
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_all_drafts') return drafts;
      if (cmd === 'approve_post') return null;
      return null;
    });
    render(<AllReposDraftsView postWizardNudge={false} onNudgeDismissed={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /approve all ready/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve all ready/i }));
    await waitFor(() => screen.getByRole('dialog'));
    fireEvent.click(screen.getByRole('button', { name: /^confirm$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('approve_post', expect.objectContaining({ postFolder: 'p1', platform: 'x' })),
    );
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('approve_post', expect.objectContaining({ postFolder: 'p2', platform: 'bluesky' })),
    );
  });
});
