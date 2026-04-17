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
    return Promise.resolve(null);
  });
  mockConfirm.mockResolvedValue(true);
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Collapsed state
// ---------------------------------------------------------------------------

describe('PostCard — collapsed state', () => {
  it('shows the trigger text', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText('Launched v2.0')).toBeInTheDocument();
  });

  it('shows repo badge', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText('my-app')).toBeInTheDocument();
  });

  it('shows platforms', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText(/x/i)).toBeInTheDocument();
  });

  it('shows only the Preview button in the header (no Approve or Delete)', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByRole('button', { name: /preview/i })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /approve/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /delete/i })).not.toBeInTheDocument();
  });

  it('falls back to first 80 chars of post_folder slug when trigger is null', () => {
    const post = makePost({ trigger: null, post_folder: 'my-interesting-post-about-things' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText('my-interesting-post-about-things')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Expanded state
// ---------------------------------------------------------------------------

describe('PostCard — expanded state', () => {
  it('shows Approve and Delete buttons only after expanding', async () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /approve/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /delete/i })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => expect(screen.getByRole('button', { name: /approve/i })).toBeInTheDocument());
    expect(screen.getByRole('button', { name: /delete/i })).toBeInTheDocument();
  });

  it('clicking Preview expands the card', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    expect(screen.getByRole('tablist')).toBeInTheDocument();
  });

  it('shows platform tabs when expanded', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    expect(screen.getByRole('tab', { name: /x/i })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /bluesky/i })).toBeInTheDocument();
  });

  it('clicking Preview again collapses the card', () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
  });

  it('loads and shows post content for the active platform when expanded', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('Timezone support is now live.');
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() =>
      expect(screen.getByText('Timezone support is now live.')).toBeInTheDocument()
    );
  });

  it('shows content for the switched platform tab', async () => {
    mockInvoke.mockImplementation((cmd, args) => {
      if (cmd === 'get_post_content') {
        const platform = (args as { platform: string }).platform;
        return Promise.resolve(platform === 'bluesky' ? 'Bluesky content here.' : 'X content here.');
      }
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => expect(screen.getByText('X content here.')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('tab', { name: /bluesky/i }));
    await waitFor(() => expect(screen.getByText('Bluesky content here.')).toBeInTheDocument());
  });
});

// ---------------------------------------------------------------------------
// Failed state
// ---------------------------------------------------------------------------

describe('PostCard — failed state', () => {
  it('always starts expanded', () => {
    const post = makePost({ status: 'failed', error: 'Scheduler timeout' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByRole('tablist')).toBeInTheDocument();
  });

  it('shows FAILED badge', () => {
    const post = makePost({ status: 'failed', error: 'Scheduler timeout' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText(/failed/i)).toBeInTheDocument();
  });

  it('shows error message', () => {
    const post = makePost({ status: 'failed', error: 'Scheduler timeout' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText('Scheduler timeout')).toBeInTheDocument();
  });

  it('shows Retry button', () => {
    const post = makePost({ status: 'failed', error: 'Scheduler timeout' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
  });

  it('shows per-platform results', () => {
    const post = makePost({
      status: 'failed',
      error: 'Partial failure',
      platform_results: { x: 'sent', bluesky: 'failed' },
      platforms: [], // no tabs, so "bluesky" only appears in results
    });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getAllByText(/bluesky/i).length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// Approve action
// ---------------------------------------------------------------------------

describe('PostCard — approve', () => {
  it('calls approve_post and fires onApproved', async () => {
    const onApproved = vi.fn();
    mockInvoke.mockResolvedValue({ success: true, platform_results: null, error: null });
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /approve/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce());
    expect(mockInvoke).toHaveBeenCalledWith('approve_post', expect.objectContaining({
      repoPath: '/path/to/repo',
      postFolder: 'post-001',
    }));
  });

  it('shows error inline on approval failure without crashing', async () => {
    mockInvoke.mockRejectedValue(new Error('Network error'));
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /approve/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    await waitFor(() => expect(screen.getByText(/network error/i)).toBeInTheDocument());
  });
});

// ---------------------------------------------------------------------------
// Dismiss action
// ---------------------------------------------------------------------------

describe('PostCard — dismiss', () => {
  it('confirms then calls delete_post and fires onDismissed', async () => {
    const onDismissed = vi.fn();
    mockInvoke.mockResolvedValue(undefined);
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={onDismissed} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /delete/i }));
    fireEvent.click(screen.getByRole('button', { name: /delete/i }));
    await waitFor(() => expect(onDismissed).toHaveBeenCalledOnce());
    expect(mockConfirm).toHaveBeenCalledOnce();
    expect(mockInvoke).toHaveBeenCalledWith('delete_post', expect.objectContaining({
      repoPath: '/path/to/repo',
      postFolder: 'post-001',
    }));
  });

  it('does not delete when user cancels the confirmation', async () => {
    mockConfirm.mockResolvedValue(false);
    const onDismissed = vi.fn();
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={onDismissed} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /delete/i }));
    fireEvent.click(screen.getByRole('button', { name: /delete/i }));
    await waitFor(() => expect(mockConfirm).toHaveBeenCalledOnce());
    expect(onDismissed).not.toHaveBeenCalled();
    expect(mockInvoke).not.toHaveBeenCalledWith('delete_post', expect.anything());
  });
});

// ---------------------------------------------------------------------------
// Inline text edit
// ---------------------------------------------------------------------------

describe('PostCard — inline edit', () => {
  it('shows Edit button when the card is expanded', async () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /edit/i })).toBeInTheDocument()
    );
  });

  it('clicking Edit shows a textarea with the loaded post content', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('Timezone support is live.');
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /edit/i }));
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    expect(screen.getByRole('textbox')).toHaveValue('Timezone support is live.');
  });

  it('Save calls update_post_content with the edited text', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('Original post.');
      if (cmd === 'update_post_content') return Promise.resolve(undefined);
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /edit/i }));
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Edited post.' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_content', expect.objectContaining({
        repoPath: '/path/to/repo',
        postFolder: 'post-001',
        platform: 'x',
        newContent: 'Edited post.',
      }))
    );
  });

  it('after a successful Save the card shows the updated content', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('Original post.');
      if (cmd === 'update_post_content') return Promise.resolve(undefined);
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /edit/i }));
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Edited post.' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() =>
      expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    );
    expect(screen.getByText('Edited post.')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Image management
// ---------------------------------------------------------------------------

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
});

// ---------------------------------------------------------------------------
// Mobile / desktop view toggle
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Keyboard shortcuts
// ---------------------------------------------------------------------------

describe('PostCard — keyboard shortcuts', () => {
  it('A key approves the focused card', async () => {
    const onApproved = vi.fn();
    mockInvoke.mockResolvedValue({ success: true });
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} isFocused />);
    const card = screen.getByRole('article');
    fireEvent.keyDown(card, { key: 'a' });
    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce());
  });

  it('D key dismisses the focused card', async () => {
    const onDismissed = vi.fn();
    mockInvoke.mockResolvedValue(undefined);
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={onDismissed} isFocused />);
    const card = screen.getByRole('article');
    fireEvent.keyDown(card, { key: 'd' });
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
});
