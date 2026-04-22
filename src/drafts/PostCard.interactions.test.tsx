// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import PostCard from './PostCard';
import type { DraftPost } from '../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
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
    ...overrides,
  };
}

describe('PostCard — image management', () => {
  it('shows Image button when expanded', async () => {
    render(<PostCard post={makePost({ image_url: null })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /^image$/i })).toBeInTheDocument()
    );
  });

  it('clicking Image shows a URL input', async () => {
    render(<PostCard post={makePost({ image_url: null })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^image$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^image$/i }));
    expect(screen.getByRole('textbox', { name: /image url/i })).toBeInTheDocument();
  });

  it('submitting a URL calls update_post_image with the URL', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('post content');
      if (cmd === 'update_post_image') return Promise.resolve(undefined);
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
      expect(mockInvoke).toHaveBeenCalledWith('update_post_image', expect.objectContaining({
        repoPath: '/path/to/repo',
        postFolder: 'post-001',
        imageUrl: 'https://example.com/og.png',
      }))
    );
  });

  it('auto-resolves an Unsplash page URL to the og:image on save', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('post content');
      if (cmd === 'fetch_og_image') return Promise.resolve('https://images.unsplash.com/photo-1554177255-61502b352de3');
      if (cmd === 'update_post_image') return Promise.resolve(undefined);
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost({ image_url: null })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^image$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^image$/i }));
    fireEvent.change(screen.getByRole('textbox', { name: /image url/i }), {
      target: { value: 'https://unsplash.com/photos/neon-signage-xv7-GlvBLFw' },
    });
    fireEvent.click(screen.getByRole('button', { name: /save image/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_image', expect.objectContaining({
        imageUrl: 'https://images.unsplash.com/photo-1554177255-61502b352de3',
      }))
    );
  });
});

describe('PostCard — image management — remove', () => {
  it('shows Image button and Remove option when image_url is set', async () => {
    render(<PostCard post={makePost({ image_url: 'https://example.com/og.png' })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => expect(screen.getByRole('button', { name: /^image$/i })).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /^image$/i }));
    expect(screen.getByRole('textbox', { name: /image url/i })).toHaveValue('https://example.com/og.png');
    expect(screen.getByRole('button', { name: /remove/i })).toBeInTheDocument();
  });

  it('clicking Remove calls update_post_image with null', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('post content');
      if (cmd === 'update_post_image') return Promise.resolve(undefined);
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost({ image_url: 'https://example.com/og.png' })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^image$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^image$/i }));
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_image', expect.objectContaining({
        repoPath: '/path/to/repo',
        postFolder: 'post-001',
        imageUrl: null,
      }))
    );
  });

  it('Cancel closes the image input without saving', async () => {
    render(<PostCard post={makePost({ image_url: null })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^image$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^image$/i }));
    expect(screen.getByRole('textbox', { name: /image url/i })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /^cancel$/i }));
    expect(screen.queryByRole('textbox', { name: /image url/i })).not.toBeInTheDocument();
  });
});

describe('PostCard — mobile / desktop view toggle', () => {
  it('shows mobile view and desktop view buttons when expanded', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    expect(screen.getByRole('button', { name: /mobile view/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /desktop view/i })).toBeInTheDocument();
  });

  it('preview container uses mobile width constraint by default', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    expect(screen.getByTestId('preview-container')).toHaveClass('max-w-[375px]');
  });

  it('desktop button removes the mobile width constraint and applies 600px constraint', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    fireEvent.click(screen.getByRole('button', { name: /desktop view/i }));
    const container = screen.getByTestId('preview-container');
    expect(container).not.toHaveClass('max-w-[375px]');
    expect(container).toHaveClass('max-w-[600px]');
  });

  it('mobile button re-applies the width constraint after switching to desktop', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    fireEvent.click(screen.getByRole('button', { name: /desktop view/i }));
    fireEvent.click(screen.getByRole('button', { name: /mobile view/i }));
    expect(screen.getByTestId('preview-container')).toHaveClass('max-w-[375px]');
  });
});

describe('PostCard — keyboard shortcuts — basic', () => {
  it('A key approves the focused card', async () => {
    const onApproved = vi.fn();
    mockInvoke.mockResolvedValue({ success: true });
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} isFocused />);
    fireEvent.keyDown(screen.getByRole('article'), { key: 'a' });
    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce());
  });

  it('D key dismisses the focused card', async () => {
    const onDismissed = vi.fn();
    mockInvoke.mockResolvedValue(undefined);
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={onDismissed} isFocused />);
    fireEvent.keyDown(screen.getByRole('article'), { key: 'd' });
    await waitFor(() => expect(onDismissed).toHaveBeenCalledOnce());
  });

  it('E key toggles expanded state', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} isFocused />);
    const card = screen.getByRole('article');
    fireEvent.keyDown(card, { key: 'e' });
    expect(screen.getByRole('tablist')).toBeInTheDocument();
    fireEvent.keyDown(card, { key: 'e' });
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
  });

  it('Escape collapses an expanded card', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} isFocused />);
    const card = screen.getByRole('article');
    fireEvent.keyDown(card, { key: 'e' });
    expect(screen.getByRole('tablist')).toBeInTheDocument();
    fireEvent.keyDown(card, { key: 'Escape' });
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
  });

  it('keyboard shortcuts are ignored when card is not focused', () => {
    const onApproved = vi.fn();
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} isFocused={false} />);
    fireEvent.keyDown(screen.getByRole('article'), { key: 'a' });
    expect(onApproved).not.toHaveBeenCalled();
  });
});

describe('PostCard — keyboard shortcuts — number keys', () => {
  it('1 key switches to first platform tab', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('content');
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost({ platforms: ['x', 'bluesky'] })} onApproved={vi.fn()} onDismissed={vi.fn()} isFocused />);
    const card = screen.getByRole('article');
    fireEvent.keyDown(card, { key: 'e' });
    await waitFor(() => screen.getByRole('tablist'));
    fireEvent.keyDown(card, { key: '2' });
    fireEvent.keyDown(card, { key: '1' });
    await waitFor(() =>
      expect(screen.getByRole('tab', { name: /x/i, selected: true })).toBeInTheDocument(),
    );
  });

  it('2 key switches to second platform tab', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('content');
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost({ platforms: ['x', 'bluesky'] })} onApproved={vi.fn()} onDismissed={vi.fn()} isFocused />);
    const card = screen.getByRole('article');
    fireEvent.keyDown(card, { key: 'e' });
    await waitFor(() => screen.getByRole('tablist'));
    fireEvent.keyDown(card, { key: '2' });
    await waitFor(() =>
      expect(screen.getByRole('tab', { name: /bluesky/i, selected: true })).toBeInTheDocument(),
    );
  });

  it('3 key does nothing when only 2 platforms', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('content');
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost({ platforms: ['x', 'bluesky'] })} onApproved={vi.fn()} onDismissed={vi.fn()} isFocused />);
    const card = screen.getByRole('article');
    fireEvent.keyDown(card, { key: 'e' });
    await waitFor(() => screen.getByRole('tablist'));
    fireEvent.keyDown(card, { key: '3' });
    expect(screen.getByRole('tab', { name: /x/i })).toBeInTheDocument();
  });

  it('R key retries a failed card', async () => {
    mockInvoke.mockResolvedValue({ success: true });
    const failedPost = makePost({ status: 'failed', error: 'Timeout' });
    render(<PostCard post={failedPost} onApproved={vi.fn()} onDismissed={vi.fn()} isFocused />);
    fireEvent.keyDown(screen.getByRole('article'), { key: 'r' });
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('retry_post', expect.anything()));
  });
});

describe('PostCard — platform default (isPlatform type guard)', () => {
  it('defaults activeTab to x when platforms is empty', async () => {
    const post = makePost({ platforms: [] });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => expect(screen.queryByRole('tab')).not.toBeInTheDocument());
  });

  it('defaults activeTab to x when platforms contains an unrecognised value', () => {
    const post = makePost({ platforms: ['instagram' as string, 'x'] });
    expect(() =>
      render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />)
    ).not.toThrow();
  });

  it('uses bluesky as activeTab when platforms starts with bluesky', async () => {
    const post = makePost({ platforms: ['bluesky', 'x'] });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => {
      const tab = screen.getByRole('tab', { name: /bluesky/i });
      expect(tab).toHaveAttribute('aria-selected', 'true');
    });
  });
});
