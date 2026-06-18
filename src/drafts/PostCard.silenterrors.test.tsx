// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import PostCard from './PostCard';
import type { DraftPost } from '../types';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));

import { invoke } from '../ipc/invoke';
import { confirm } from '@tauri-apps/plugin-dialog';
const mockInvoke = vi.mocked(invoke);
const mockConfirm = vi.mocked(confirm);

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_post_content') return Promise.resolve('');
    if (cmd === 'get_attribution') return Promise.resolve(true);
    return Promise.resolve(null);
  });
  mockConfirm.mockResolvedValue(true);
});

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

describe('PostCard — Fix 4: redraft queue overwrite blocked', () => {
  it('shows "already queued" error message in UI when queue_redraft returns that error', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'get_attribution') return Promise.resolve(true);
      if (cmd === 'queue_redraft') return Promise.reject(new Error('A redraft is already queued. Cancel the existing redraft first.'));
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByPlaceholderText(/ask the llm to revise/i));
    fireEvent.change(screen.getByPlaceholderText(/ask the llm to revise/i), {
      target: { value: 'make it shorter' },
    });
    fireEvent.click(screen.getByRole('button', { name: /queue for redraft/i }));
    await waitFor(() =>
      expect(screen.getByText(/A redraft is already queued/i)).toBeInTheDocument(),
    );
  });
});

describe('PostCard — cancel_redraft error (HIGH-5)', () => {
  it('shows error alert when cancel_redraft fails', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'queue_redraft') return Promise.resolve(null);
      if (cmd === 'cancel_redraft') return Promise.reject(new Error('cancel failed'));
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByPlaceholderText(/ask the llm to revise/i));
    fireEvent.change(screen.getByPlaceholderText(/ask the llm to revise/i), {
      target: { value: 'make it shorter' },
    });
    fireEvent.click(screen.getByRole('button', { name: /queue for redraft/i }));
    await waitFor(() => screen.getByRole('button', { name: /cancel redraft/i }));
    fireEvent.click(screen.getByRole('button', { name: /cancel redraft/i }));
    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent(/cancel failed/i),
    );
  });
});

describe('PostCard — image save/remove errors (HIGH-6)', () => {
  it('shows error alert when update_post_image fails on save', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('content');
      if (cmd === 'update_post_image') return Promise.reject(new Error('disk full'));
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost({ image_url: null })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^image$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^image$/i }));
    fireEvent.change(screen.getByRole('textbox', { name: /image url/i }), {
      target: { value: 'https://example.com/og.png' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^save image$/i }));
    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent(/disk full/i),
    );
  });

  it('shows error alert when update_post_image fails on remove', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('content');
      if (cmd === 'update_post_image') return Promise.reject(new Error('disk full'));
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost({ image_url: 'https://example.com/og.png' })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^image$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^image$/i }));
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent(/disk full/i),
    );
  });
});
