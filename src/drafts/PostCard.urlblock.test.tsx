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
    if (cmd === 'has_unsplash_key') return Promise.resolve(true);
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

async function openUrlMode() {
  // Expand card
  fireEvent.click(screen.getByRole('button', { name: /preview/i }));
  // Click Image button in CardActions
  const imageBtn = await screen.findByRole('button', { name: /^image$/i });
  fireEvent.click(imageBtn);
  // Switch to URL mode tab
  const urlTab = await screen.findByRole('button', { name: /^url$/i });
  fireEvent.click(urlTab);
}

// 21.8.3, 21.8.13: pasting images.unsplash.com URL in URL mode shows blocking warning
describe('PostCard — URL blocking for Unsplash direct paste', () => {
  it('shows blocking warning when images.unsplash.com URL is entered in URL mode', async () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await openUrlMode();
    const urlInput = await screen.findByRole('textbox', { name: /image url/i });
    fireEvent.change(urlInput, { target: { value: 'https://images.unsplash.com/photo-abc' } });
    expect(screen.getByText(/compliance requires selecting via search/i)).toBeInTheDocument();
  });

  // 21.8.14: pasting plus.unsplash.com URL shows blocking warning
  it('shows blocking warning when plus.unsplash.com URL is entered in URL mode', async () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await openUrlMode();
    const urlInput = await screen.findByRole('textbox', { name: /image url/i });
    fireEvent.change(urlInput, { target: { value: 'https://plus.unsplash.com/photo-abc' } });
    expect(screen.getByText(/compliance requires selecting via search/i)).toBeInTheDocument();
  });

  // 21.8.3: Save button is disabled when Unsplash URL detected
  it('Save button is disabled when Unsplash URL is in URL input', async () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await openUrlMode();
    const urlInput = await screen.findByRole('textbox', { name: /image url/i });
    fireEvent.change(urlInput, { target: { value: 'https://images.unsplash.com/photo-abc' } });
    const saveBtn = screen.getByRole('button', { name: /save image/i });
    expect(saveBtn).toBeDisabled();
  });

  // Non-Unsplash URL should not show blocking warning
  it('does not show blocking warning for a normal image URL', async () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    await openUrlMode();
    const urlInput = await screen.findByRole('textbox', { name: /image url/i });
    fireEvent.change(urlInput, { target: { value: 'https://example.com/image.png' } });
    expect(screen.queryByText(/compliance requires selecting via search/i)).not.toBeInTheDocument();
  });

  // 21.8.12: Search tab hidden when no Unsplash key configured
  it('shows URL mode directly when no Unsplash key is configured', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('Draft text here');
      if (cmd === 'get_attribution') return Promise.resolve(true);
      if (cmd === 'has_unsplash_key') return Promise.resolve(false);
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    const imageBtn = await screen.findByRole('button', { name: /^image$/i });
    fireEvent.click(imageBtn);
    // No search tab should be visible — URL input should be directly available
    expect(screen.queryByRole('button', { name: /^search$/i })).not.toBeInTheDocument();
    expect(await screen.findByRole('textbox', { name: /image url/i })).toBeInTheDocument();
  });
});
