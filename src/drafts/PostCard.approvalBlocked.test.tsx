// SPDX-License-Identifier: BUSL-1.1
// Tests for the license approval-block CTA (checklist 24.4.11): approve_post
// rejects with a structured { kind: 'blocked', status, is_owner, days_remaining }
// error, and PostCard must render different copy/actions by status and role.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import PostCard from './PostCard';
import type { DraftPost } from '../types';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
const mockInvoke = vi.mocked(invoke);
const mockOpenUrl = vi.mocked(openUrl);

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
    project_id: null,
    platform: 'x',
    text: '',
    ...overrides,
  };
}

function mockApproveRejects(blocked: { status: string; is_owner: boolean; days_remaining: number | null }) {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_post_content') return Promise.resolve('');
    if (cmd === 'get_attribution') return Promise.resolve(true);
    if (cmd === 'approve_post') return Promise.reject({ kind: 'blocked', ...blocked });
    return Promise.resolve(null);
  });
}

async function clickApprove() {
  fireEvent.click(screen.getByRole('button', { name: /preview/i }));
  await waitFor(() => screen.getByRole('button', { name: /^approve$/i }));
  fireEvent.click(screen.getByRole('button', { name: /^approve$/i }));
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('PostCard — approval block — inactive', () => {
  it('owner sees a Reactivate CTA', async () => {
    mockApproveRejects({ status: 'inactive', is_owner: true, days_remaining: null });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await clickApprove();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /reactivate to resume posting/i })).toBeInTheDocument(),
    );
  });

  it('collaborator sees a read-only paused message with no action', async () => {
    mockApproveRejects({ status: 'inactive', is_owner: false, days_remaining: null });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await clickApprove();
    await waitFor(() => expect(screen.getByText(/paused by its owner/i)).toBeInTheDocument());
    expect(screen.queryByRole('button', { name: /reactivate/i })).not.toBeInTheDocument();
  });

  it('clicking Reactivate opens the web dashboard', async () => {
    mockApproveRejects({ status: 'inactive', is_owner: true, days_remaining: null });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await clickApprove();
    await waitFor(() => screen.getByRole('button', { name: /reactivate to resume posting/i }));
    fireEvent.click(screen.getByRole('button', { name: /reactivate to resume posting/i }));
    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/dashboard'));
  });
});

describe('PostCard — approval block — payment_failed', () => {
  it('owner sees billing copy disclosing days remaining', async () => {
    mockApproveRejects({ status: 'payment_failed', is_owner: true, days_remaining: 5 });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await clickApprove();
    await waitFor(() => expect(screen.getByText(/5 days? left/i)).toBeInTheDocument());
    expect(screen.getByText(/update billing/i)).toBeInTheDocument();
  });

  it('collaborator sees take-over-billing copy, not the owner days-remaining copy', async () => {
    mockApproveRejects({ status: 'payment_failed', is_owner: false, days_remaining: null });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await clickApprove();
    await waitFor(() => expect(screen.getByText(/take over billing/i)).toBeInTheDocument());
    expect(screen.queryByText(/days? left/i)).not.toBeInTheDocument();
  });
});

describe('PostCard — approval block — unlicensed', () => {
  it('shows take-over-billing-or-read-only copy', async () => {
    mockApproveRejects({ status: 'unlicensed', is_owner: false, days_remaining: null });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await clickApprove();
    await waitFor(() => expect(screen.getByText(/take over billing or continue read-only/i)).toBeInTheDocument());
  });
});

describe('PostCard — approval block — non-blocked errors unaffected', () => {
  it('a plain approve_post failure still renders as a normal error message', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'get_attribution') return Promise.resolve(true);
      if (cmd === 'approve_post') return Promise.reject({ kind: 'message', message: 'Post folder does not exist' });
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await clickApprove();
    await waitFor(() => expect(screen.getByText(/post folder does not exist/i)).toBeInTheDocument());
    expect(screen.queryByRole('button', { name: /reactivate/i })).not.toBeInTheDocument();
  });
});
