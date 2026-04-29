// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import PostCard from './PostCard';
import type { DraftPost } from '../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));

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
    trigger: 'Launched v2.0',
    platform_results: null,
    error: null,
    image_url: null,
    llm_model: null,
    created_at: null,
    ...overrides,
  };
}

function setupInvoke(fallbackProvider: string | null = null) {
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'get_post_content') return '';
    if (cmd === 'get_attribution') return true;
    if (cmd === 'get_mastodon_connected_instance') return null;
    if (cmd === 'approve_post') {
      return { success: true, platform_results: { x: 'success' }, error: null, fallback_provider: fallbackProvider };
    }
    return null;
  });
}

describe('PostCard — fallback provider banner (§13.2.3)', () => {
  it('shows fallback banner when approve_post returns fallback_provider', async () => {
    setupInvoke('zernio');
    const onApproved = vi.fn();
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /approve/i }));
    fireEvent.click(screen.getByRole('button', { name: /^approve$/i }));

    await waitFor(() =>
      expect(screen.getByText(/posted via zernio/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/primary provider has reached its limit/i)).toBeInTheDocument();
    expect(onApproved).not.toHaveBeenCalled();
  });

  it('calls onApproved when user dismisses the fallback banner', async () => {
    setupInvoke('zernio');
    const onApproved = vi.fn();
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^approve$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^approve$/i }));

    await waitFor(() => screen.getByText(/posted via zernio/i));
    fireEvent.click(screen.getByRole('button', { name: /got it/i }));

    expect(onApproved).toHaveBeenCalledOnce();
  });

  it('calls onApproved immediately when no fallback was used', async () => {
    setupInvoke(null);
    const onApproved = vi.fn();
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^approve$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^approve$/i }));

    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce());
    expect(screen.queryByText(/posted via/i)).not.toBeInTheDocument();
  });
});
