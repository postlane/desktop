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

async function openImagePanel() {
  fireEvent.click(screen.getByRole('button', { name: /preview/i }));
  const imageBtn = await screen.findByRole('button', { name: /^image$/i });
  fireEvent.click(imageBtn);
}

// 21.8.3, 21.8.13: pasting images.unsplash.com URL in URL input shows blocking warning
describe('PostCard — URL blocking for Unsplash direct paste', () => {
  it('shows blocking warning when images.unsplash.com URL is entered', async () => {
    render(<PostCard post={makePost()} hasUnsplashKey onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await openImagePanel();
    const urlInput = await screen.findByRole('textbox', { name: /image url/i });
    fireEvent.change(urlInput, { target: { value: 'https://images.unsplash.com/photo-abc' } });
    expect(screen.getByText(/use the search above/i)).toBeInTheDocument();
  });

  // 21.8.14: pasting plus.unsplash.com URL shows blocking warning
  it('shows blocking warning when plus.unsplash.com URL is entered', async () => {
    render(<PostCard post={makePost()} hasUnsplashKey onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await openImagePanel();
    const urlInput = await screen.findByRole('textbox', { name: /image url/i });
    fireEvent.change(urlInput, { target: { value: 'https://plus.unsplash.com/photo-abc' } });
    expect(screen.getByText(/use the search above/i)).toBeInTheDocument();
  });

  // 21.8.3: Save button is disabled when Unsplash URL detected
  it('Save button is disabled when Unsplash URL is in URL input', async () => {
    render(<PostCard post={makePost()} hasUnsplashKey onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await openImagePanel();
    const urlInput = await screen.findByRole('textbox', { name: /image url/i });
    fireEvent.change(urlInput, { target: { value: 'https://images.unsplash.com/photo-abc' } });
    expect(screen.getByRole('button', { name: /save image/i })).toBeDisabled();
  });

  it('does not show blocking warning for a normal image URL', async () => {
    render(<PostCard post={makePost()} hasUnsplashKey onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await openImagePanel();
    const urlInput = await screen.findByRole('textbox', { name: /image url/i });
    fireEvent.change(urlInput, { target: { value: 'https://example.com/image.png' } });
    expect(screen.queryByText(/use the search above/i)).not.toBeInTheDocument();
  });

  // 21.8.12: Search section always shown (feature-flagged by keyring key at ship time)
  it('shows Unsplash search section regardless of hasUnsplashKey prop', async () => {
    render(<PostCard post={makePost()} hasUnsplashKey={false} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await openImagePanel();
    expect(await screen.findByRole('searchbox', { name: /search unsplash/i })).toBeInTheDocument();
    expect(await screen.findByRole('textbox', { name: /image url/i })).toBeInTheDocument();
  });
});
