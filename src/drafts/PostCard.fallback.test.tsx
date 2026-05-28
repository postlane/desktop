// SPDX-License-Identifier: BUSL-1.1
// approve_post returns Result<(), String> which serialises to null in JavaScript.
// The fallback_provider feature was planned but never implemented in the Rust command.
// These tests verify the correct behavior: null from approve_post means success.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import PostCard from './PostCard';
import type { DraftPost } from '../types';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));

import { invoke } from '../ipc/invoke';
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
    trigger: 'Launched v2.0',
    platform_results: null,
    error: null,
    image_url: null,
    llm_model: null,
    created_at: null,
    project_id: null,
    platform: 'x',
    text: '',
    ...overrides,
  };
}

describe('PostCard — approve_post null result (CRITICAL-1)', () => {
  // approve_post is Result<(), String> — returns null on success, throws on failure.
  // The old SendResult / fallback_provider API was never implemented in Rust.

  it('null result from approve_post counts as success — no error shown', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_post_content') return '';
      if (cmd === 'get_attribution') return true;
      if (cmd === 'get_mastodon_connected_instance') return null;
      if (cmd === 'approve_post') return null;
      return null;
    });
    const onApproved = vi.fn();
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /approve/i }));
    fireEvent.click(screen.getByRole('button', { name: /^approve$/i }));
    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce(), { timeout: 5000 });
    // No fallback banner appears (approve_post does not return fallback_provider)
    expect(screen.queryByText(/posted via/i)).not.toBeInTheDocument();
  }, 10_000);

  it('calls onApproved after success when approve_post returns null', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_post_content') return '';
      if (cmd === 'get_attribution') return true;
      if (cmd === 'get_mastodon_connected_instance') return null;
      if (cmd === 'approve_post') return null;
      return null;
    });
    const onApproved = vi.fn();
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^approve$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^approve$/i }));
    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce(), { timeout: 5000 });
  }, 10_000);

  it('no error shown when approve_post succeeds with null', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_post_content') return '';
      if (cmd === 'get_attribution') return true;
      if (cmd === 'get_mastodon_connected_instance') return null;
      if (cmd === 'approve_post') return null;
      return null;
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^approve$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^approve$/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('approve_post', expect.anything()));
    await new Promise((r) => setTimeout(r, 100));
    expect(screen.queryByText(/cannot read/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/typeerror/i)).not.toBeInTheDocument();
  });
});
