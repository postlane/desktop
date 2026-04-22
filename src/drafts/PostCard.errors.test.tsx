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

describe('PostCard — error paths — content and fetch', () => {
  it('update_post_content failure does not crash', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('Original.');
      if (cmd === 'update_post_content') return Promise.reject(new Error('write failed'));
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /edit/i }));
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Changed.' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_content', expect.anything()),
    );
    expect(screen.getByRole('article')).toBeInTheDocument();
  });

  it('shows error when fetch_og_image returns null (no image found)', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('content');
      if (cmd === 'fetch_og_image') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost({ image_url: null })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^image$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^image$/i }));
    fireEvent.change(screen.getByRole('textbox', { name: /image url/i }), {
      target: { value: 'https://unsplash.com/photos/some-photo' },
    });
    fireEvent.click(screen.getByRole('button', { name: /save image/i }));
    await waitFor(() =>
      expect(screen.getByText(/no image found on that page/i)).toBeInTheDocument(),
    );
  });

  it('shows error when fetch_og_image throws', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('content');
      if (cmd === 'fetch_og_image') return Promise.reject(new Error('timeout'));
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost({ image_url: null })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^image$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^image$/i }));
    fireEvent.change(screen.getByRole('textbox', { name: /image url/i }), {
      target: { value: 'https://unsplash.com/photos/some-photo' },
    });
    fireEvent.click(screen.getByRole('button', { name: /save image/i }));
    await waitFor(() =>
      expect(screen.getByText(/timeout/i)).toBeInTheDocument(),
    );
  });
});

describe('PostCard — error paths — image save', () => {
  it('malformed image URL falls through to fetch_og_image', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_post_content') return Promise.resolve('content');
      if (cmd === 'fetch_og_image') return Promise.resolve('https://images.example.com/photo.jpg');
      if (cmd === 'update_post_image') return Promise.resolve(undefined);
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost({ image_url: null })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /^image$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^image$/i }));
    fireEvent.change(screen.getByRole('textbox', { name: /image url/i }), {
      target: { value: 'https://' },
    });
    fireEvent.click(screen.getByRole('button', { name: /save image/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('fetch_og_image', { url: 'https://' }),
    );
  });

  it('update_post_image failure on save does not crash', async () => {
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
      expect(mockInvoke).toHaveBeenCalledWith('update_post_image', expect.anything()),
    );
  });

  it('update_post_image failure on remove does not crash', async () => {
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
      expect(mockInvoke).toHaveBeenCalledWith('update_post_image', expect.anything()),
    );
  });
});

describe('PostCard — error paths — delete and retry', () => {
  it('delete_post failure does not crash', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'delete_post') return Promise.reject(new Error('permission denied'));
      return Promise.resolve(null);
    });
    const onDismissed = vi.fn();
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={onDismissed} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /delete/i }));
    fireEvent.click(screen.getByRole('button', { name: /delete/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('delete_post', expect.anything()),
    );
    expect(onDismissed).not.toHaveBeenCalled();
  });

  it('retry_post failure does not crash', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'retry_post') return Promise.reject(new Error('network error'));
      return Promise.resolve(null);
    });
    const onApproved = vi.fn();
    const failedPost = makePost({ status: 'failed', error: 'Timeout' });
    render(<PostCard post={failedPost} onApproved={onApproved} onDismissed={vi.fn()} isFocused />);
    fireEvent.keyDown(screen.getByRole('article'), { key: 'r' });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('retry_post', expect.anything()),
    );
    expect(onApproved).not.toHaveBeenCalled();
  });
});

describe('PostCard — queue for redraft — basics', () => {
  it('renders a "Queue for redraft" button in the expanded state', async () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /queue for redraft/i })).toBeInTheDocument(),
    );
  });

  it('button is disabled when instruction field is empty', async () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('button', { name: /queue for redraft/i }));
    expect(screen.getByRole('button', { name: /queue for redraft/i })).toBeDisabled();
  });

  it('button is enabled when instruction has text', async () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByPlaceholderText(/ask the llm to revise/i));
    fireEvent.change(screen.getByPlaceholderText(/ask the llm to revise/i), {
      target: { value: 'make it shorter' },
    });
    expect(screen.getByRole('button', { name: /queue for redraft/i })).not.toBeDisabled();
  });

  it('calls queue_redraft with correct args when button is clicked', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'queue_redraft') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    const post = makePost({ repo_path: '/repos/my-app', post_folder: 'post-001' });
    render(<PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByPlaceholderText(/ask the llm to revise/i));
    fireEvent.change(screen.getByPlaceholderText(/ask the llm to revise/i), {
      target: { value: 'make it punchier' },
    });
    fireEvent.click(screen.getByRole('button', { name: /queue for redraft/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('queue_redraft', {
        repoPath: '/repos/my-app',
        postFolder: 'post-001',
        instruction: 'make it punchier',
      }),
    );
  });
});

describe('PostCard — queue for redraft — banner and cancel', () => {
  it('shows a banner after successful queue_redraft', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'queue_redraft') return Promise.resolve(null);
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
      expect(screen.getByText(/queued for redraft/i)).toBeInTheDocument(),
    );
  });

  it('shows "Cancel redraft" button after queueing', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'queue_redraft') return Promise.resolve(null);
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
      expect(screen.getByRole('button', { name: /cancel redraft/i })).toBeInTheDocument(),
    );
  });

  it('clicking "Cancel redraft" invokes cancel_redraft and hides the banner', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'queue_redraft') return Promise.resolve(null);
      if (cmd === 'cancel_redraft') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost({ repo_path: '/repos/my-app' })} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByPlaceholderText(/ask the llm to revise/i));
    fireEvent.change(screen.getByPlaceholderText(/ask the llm to revise/i), {
      target: { value: 'make it punchier' },
    });
    fireEvent.click(screen.getByRole('button', { name: /queue for redraft/i }));
    await waitFor(() => screen.getByRole('button', { name: /cancel redraft/i }));
    fireEvent.click(screen.getByRole('button', { name: /cancel redraft/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('cancel_redraft', { repoPath: '/repos/my-app' }),
    );
    await waitFor(() =>
      expect(screen.queryByText(/queued for redraft/i)).not.toBeInTheDocument(),
    );
  });
});

describe('PostCard — queue for redraft — confirm dialog', () => {
  it('shows a confirm dialog before queuing redraft', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'queue_redraft') return Promise.resolve(null);
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
      expect(mockConfirm).toHaveBeenCalledWith(
        expect.stringContaining('make it shorter'),
        expect.objectContaining({ title: 'Confirm redraft' }),
      ),
    );
  });

  it('does not queue when user cancels the confirm dialog', async () => {
    mockConfirm.mockResolvedValue(false);
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByPlaceholderText(/ask the llm to revise/i));
    fireEvent.change(screen.getByPlaceholderText(/ask the llm to revise/i), {
      target: { value: 'make it shorter' },
    });
    fireEvent.click(screen.getByRole('button', { name: /queue for redraft/i }));
    await waitFor(() => expect(mockConfirm).toHaveBeenCalledOnce());
    expect(mockInvoke).not.toHaveBeenCalledWith('queue_redraft', expect.anything());
    expect(screen.queryByText(/queued for redraft/i)).not.toBeInTheDocument();
  });
});

describe('PostCard — Fix 3: redraft instruction input constraints', () => {
  it('redraft instruction input has maxLength of 10000', async () => {
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByRole('searchbox', { name: /redraft instruction/i }));
    const input = screen.getByRole('searchbox', { name: /redraft instruction/i });
    expect(input).toHaveAttribute('maxlength', '10000');
  });

  it('shows error message in UI when queue_redraft invoke throws', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_post_content') return Promise.resolve('');
      if (cmd === 'get_attribution') return Promise.resolve(true);
      if (cmd === 'queue_redraft') return Promise.reject(new Error('Failed to write file'));
      return Promise.resolve(null);
    });
    render(<PostCard post={makePost()} onApproved={vi.fn()} onDismissed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    await waitFor(() => screen.getByPlaceholderText(/ask the llm to revise/i));
    fireEvent.change(screen.getByPlaceholderText(/ask the llm to revise/i), {
      target: { value: 'make it punchier' },
    });
    fireEvent.click(screen.getByRole('button', { name: /queue for redraft/i }));
    await waitFor(() =>
      expect(screen.getByText(/Failed to write file/i)).toBeInTheDocument(),
    );
  });
});

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
