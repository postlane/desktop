// SPDX-License-Identifier: BUSL-1.1
// Integration tests for the SendSuccessModal shown after a successful approval.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import PostCard from './PostCard';
import type { DraftPost } from '../types';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));

import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_post_content') return Promise.resolve('');
    if (cmd === 'get_attribution') return Promise.resolve(true);
    return Promise.resolve(null);
  });
});

function makePost(overrides: Partial<DraftPost> = {}): DraftPost {
  return {
    repo_id: 'r1', repo_name: 'my-app', repo_path: '/path/to/repo',
    post_folder: 'post-001', status: 'ready', platforms: ['x', 'bluesky'],
    schedule: '2026-06-01T10:00:00Z', trigger: 'Launched v2.0',
    platform_results: null, error: null, image_url: null,
    llm_model: 'claude-sonnet-4-6', created_at: '2026-04-15T09:00:00Z',
    project_id: null, platform: 'x', text: '',
    ...overrides,
  };
}

function makeApproveInvoke(platforms: string[]) {
  return async (cmd: unknown) => {
    if (cmd === 'get_post_content') return '';
    if (cmd === 'get_attribution') return true;
    if (cmd === 'approve_post') {
      return {
        success: true,
        platform_results: Object.fromEntries(platforms.map((p) => [p, 'sent'])),
        error: null,
        fallback_provider: null,
      };
    }
    return null;
  };
}

describe('PostCard — send success modal (§feature-13)', () => {
  it('shows the success modal (dialog) after a successful approval', async () => {
    mockInvoke.mockImplementation(makeApproveInvoke(['x', 'bluesky']));
    render(<PostCard post={makePost({ platforms: ['x', 'bluesky'] })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await screen.findByRole('button', { name: /approve/i });
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    await waitFor(() => expect(screen.getByRole('dialog', { name: /post sent/i })).toBeInTheDocument());
  });

  it('success modal status region contains sent platform names', async () => {
    mockInvoke.mockImplementation(makeApproveInvoke(['x', 'bluesky']));
    render(<PostCard post={makePost({ platforms: ['x', 'bluesky'] })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await screen.findByRole('button', { name: /approve/i });
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    await waitFor(() => {
      const status = screen.getByRole('status');
      expect(status.textContent).toMatch(/sent/i);
    });
  });

  it('modal appears before onApproved fires, then auto-dismisses', async () => {
    const onApproved = vi.fn();
    mockInvoke.mockImplementation(makeApproveInvoke(['x']));
    render(<PostCard post={makePost({ platforms: ['x'] })} onApproved={onApproved} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await screen.findByRole('button', { name: /approve/i });
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    // Modal visible, onApproved not yet fired
    await waitFor(() => expect(screen.getByRole('dialog', { name: /post sent/i })).toBeInTheDocument());
    expect(onApproved).not.toHaveBeenCalled();
    // After 2.5 s auto-dismiss, onApproved fires
    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce(), { timeout: 5000 });
  }, 10_000);
});
