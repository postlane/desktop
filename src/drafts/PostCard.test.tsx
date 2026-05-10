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
    project_id: null,
    platform: 'x',
    text: '',
    ...overrides,
  };
}

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

  it('shows the llm_model label when present (§review-product-high)', () => {
    render(<PostCard post={makePost({ llm_model: 'claude-sonnet-4-6' })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText(/claude-sonnet-4-6/i)).toBeInTheDocument();
  });

  it('does not show a model label when llm_model is null', () => {
    render(<PostCard post={makePost({ llm_model: null })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.queryByLabelText(/model/i)).not.toBeInTheDocument();
  });
});

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
      platforms: [],
    });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getAllByText(/bluesky/i).length).toBeGreaterThan(0);
  });
});

describe('PostCard — approve', () => {
  it('calls approve_post and fires onApproved', async () => {
    const onApproved = vi.fn();
    mockInvoke.mockResolvedValue({ success: true, platform_results: null, error: null });
    render(<PostCard post={makePost()} onApproved={onApproved} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /approve/i }));
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce(), { timeout: 2500 });
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

describe('PostCard — auto-schedule badge (§fix-12)', () => {
  it('shows auto badge when schedule_source is default', () => {
    render(<PostCard post={makePost({ schedule_source: 'default' })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.getByText('auto')).toBeInTheDocument();
  });

  it('does not show auto badge when schedule_source is user', () => {
    render(<PostCard post={makePost({ schedule_source: 'user' })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.queryByText('auto')).not.toBeInTheDocument();
  });

  it('does not show auto badge when schedule_source is null', () => {
    render(<PostCard post={makePost({ schedule_source: null })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    expect(screen.queryByText('auto')).not.toBeInTheDocument();
  });
});

describe('PostCard — success notice after approve (§review-critical)', () => {
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

  it('shows a success notice after a successful approval', async () => {
    mockInvoke.mockImplementation(makeApproveInvoke(['x', 'bluesky']));
    render(<PostCard post={makePost({ platforms: ['x', 'bluesky'] })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await screen.findByRole('button', { name: /approve/i });
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    await waitFor(() => expect(screen.getByRole('status')).toBeInTheDocument());
  });

  it('success notice lists the platforms that were sent', async () => {
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

  it('calls onApproved after the success notice has been shown', async () => {
    const onApproved = vi.fn();
    mockInvoke.mockImplementation(makeApproveInvoke(['x']));
    render(<PostCard post={makePost({ platforms: ['x'] })} onApproved={onApproved} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await screen.findByRole('button', { name: /approve/i });
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    // Notice appears before onApproved fires
    await waitFor(() => expect(screen.getByRole('status')).toBeInTheDocument());
    expect(onApproved).not.toHaveBeenCalled();
    // After the deferral window, onApproved fires
    await waitFor(() => expect(onApproved).toHaveBeenCalledOnce(), { timeout: 2500 });
  }, 10_000);
});

// §review-product-medium — image URL error type distinction
async function openImageInput() {
  render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
  fireEvent.click(screen.getByRole('button', { name: /preview/i }));
  await screen.findByRole('button', { name: /approve/i });
  fireEvent.click(screen.getByRole('button', { name: /image/i }));
  await screen.findByRole('button', { name: /save image/i });
  const input = screen.getByRole('textbox', { name: /image url/i });
  fireEvent.change(input, { target: { value: 'https://example.com/post' } });
}

describe('PostCard — OG image fetch error types (§review-product-medium)', () => {
  it('shows "No image found" when fetch_og_image returns null', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'fetch_og_image') return Promise.resolve(null);
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'get_attribution') return Promise.resolve(true);
      return Promise.resolve(null);
    });
    await openImageInput();
    fireEvent.click(screen.getByRole('button', { name: /save image/i }));
    await waitFor(() => expect(screen.getByText(/no image found/i)).toBeInTheDocument());
  });

  it('shows "Could not reach this URL" when fetch_og_image throws with unreachable prefix', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'fetch_og_image') return Promise.reject(new Error('unreachable: connection refused'));
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'get_attribution') return Promise.resolve(true);
      return Promise.resolve(null);
    });
    await openImageInput();
    fireEvent.click(screen.getByRole('button', { name: /save image/i }));
    await waitFor(() => expect(screen.getByText(/could not reach this url/i)).toBeInTheDocument());
  });

  it('shows raw error for non-network errors (e.g., URL validation)', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'fetch_og_image') return Promise.reject(new Error('URL must start with https://'));
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'get_attribution') return Promise.resolve(true);
      return Promise.resolve(null);
    });
    await openImageInput();
    fireEvent.click(screen.getByRole('button', { name: /save image/i }));
    await waitFor(() => expect(screen.getByText(/url must start with https/i)).toBeInTheDocument());
  });
});
