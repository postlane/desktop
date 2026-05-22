// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
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
    if (cmd === 'get_post_content') return Promise.resolve('Draft text here');
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
    platforms: ['x'],
    schedule: null,
    trigger: 'Test trigger',
    platform_results: null,
    error: null,
    image_url: null,
    llm_model: null,
    created_at: '2026-04-15T09:00:00Z',
    project_id: null,
    platform: 'x',
    text: 'Test content',
    ...overrides,
  };
}

function expandCard() {
  fireEvent.click(screen.getByRole('button', { name: /preview/i }));
}

// 21.8.19: attribution rendered when image_attribution is present
describe('PostCard — image attribution', () => {
  it('shows "Photo by NAME on Unsplash" when image_attribution is present', async () => {
    const post = makePost({
      image_url: 'https://images.unsplash.com/photo-abc',
      image_attribution: {
        photographer_name: 'Jane Doe',
        photographer_url: 'https://unsplash.com/@janedoe',
      },
    });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expandCard();
    expect(await screen.findByText(/Photo by/i)).toBeInTheDocument();
    expect(screen.getByText(/Jane Doe/)).toBeInTheDocument();
    expect(screen.getByText(/Unsplash/)).toBeInTheDocument();
  });

  // 21.8.19: attribution link points to photographer_url
  it('attribution link points to photographer_url', async () => {
    const post = makePost({
      image_url: 'https://images.unsplash.com/photo-abc',
      image_attribution: {
        photographer_name: 'Jane Doe',
        photographer_url: 'https://unsplash.com/@janedoe',
      },
    });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expandCard();
    const link = await screen.findByRole('link', { name: /Jane Doe/i });
    expect(link).toHaveAttribute('href', 'https://unsplash.com/@janedoe');
  });

  // 21.8.20: no attribution when image_attribution is null
  it('shows no "Photo by" when image_attribution is null', () => {
    const post = makePost({
      image_url: 'https://images.unsplash.com/photo-abc',
      image_attribution: null,
    });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expandCard();
    expect(screen.queryByText(/Photo by/i)).not.toBeInTheDocument();
  });

  // 21.8.20: no attribution when image_attribution is absent
  it('shows no "Photo by" when image_attribution field is absent', () => {
    const post = makePost({ image_url: 'https://images.unsplash.com/photo-abc' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expandCard();
    expect(screen.queryByText(/Photo by/i)).not.toBeInTheDocument();
  });
});
