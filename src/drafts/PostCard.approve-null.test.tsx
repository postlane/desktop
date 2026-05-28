// SPDX-License-Identifier: BUSL-1.1
// Tests for CRITICAL 1: approve_post returns null (not an object) from Tauri,
// and the platform argument must be passed to invoke.

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
    repo_id: 'r1',
    repo_name: 'my-app',
    repo_path: '/path/to/repo',
    post_folder: 'post-001',
    status: 'ready',
    platforms: ['x'],
    schedule: null,
    trigger: 'Launched v2.0',
    platform_results: null,
    error: null,
    image_url: null,
    llm_model: null,
    created_at: '2026-04-15T09:00:00Z',
    project_id: null,
    platform: 'x',
    text: '',
    ...overrides,
  };
}

describe('PostCard — approve_post returns null (CRITICAL-1)', () => {
  it('approval succeeds without error when approve_post returns null', async () => {
    // approve_post returns Result<(), String> which serialises to null in JS.
    // The current code reads result.fallback_provider on null — this must not throw.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'get_attribution') return Promise.resolve(true);
      if (cmd === 'approve_post') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    const onApproved = vi.fn();
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /approve/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    // Should not show an error
    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce(), { timeout: 5000 });
    expect(screen.queryByText(/typeerror/i)).not.toBeInTheDocument();
  }, 10_000);

  it('no approveError is shown when approve_post returns null', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'get_attribution') return Promise.resolve(true);
      if (cmd === 'approve_post') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /approve/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    // Wait for the invoke to complete
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('approve_post', expect.anything()));
    // Give time for any error state to appear
    await new Promise((r) => setTimeout(r, 100));
    expect(screen.queryByText(/cannot read/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/undefined/i)).not.toBeInTheDocument();
  });

  it('approve_post invoke includes the platform argument', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'get_attribution') return Promise.resolve(true);
      if (cmd === 'approve_post') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    const post = makePost({ platform: 'x', repo_path: '/my/repo', post_folder: 'post-001' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /approve/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('approve_post', expect.objectContaining({
        platform: 'x',
      })),
    );
  });
});
