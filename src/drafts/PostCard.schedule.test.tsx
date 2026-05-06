// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { TimezoneContext } from '../TimezoneContext';
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
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'get_post_content') return '';
    if (cmd === 'get_attribution') return true;
    if (cmd === 'update_post_schedule') return null;
    return null;
  });
});

function makePost(overrides: Partial<DraftPost> = {}): DraftPost {
  return {
    repo_id: 'r1', repo_name: 'my-app', repo_path: '/path/to/repo',
    post_folder: 'post-001', status: 'ready', platforms: ['x'],
    schedule: '2026-06-01T10:00:00Z', trigger: 'Launched v2.0',
    platform_results: null, error: null, image_url: null,
    llm_model: 'claude-sonnet-4-6', created_at: '2026-04-15T09:00:00Z',
    ...overrides,
  };
}

function renderExpanded(post: DraftPost) {
  render(
    <TimezoneContext.Provider value="UTC">
      <PostCard post={post} onApproved={vi.fn()} onDismissed={vi.fn()} />
    </TimezoneContext.Provider>,
  );
  fireEvent.click(screen.getByRole('button', { name: /preview/i }));
}

describe('PostCard — schedule row — display (§17.5)', () => {
  it('shows a datetime-local input pre-populated from post.schedule', async () => {
    renderExpanded(makePost({ schedule: '2026-06-01T10:00:00Z' }));
    const input = await screen.findByLabelText(/scheduled time/i) as HTMLInputElement;
    expect(input.value).toBe('2026-06-01T10:00');
  });

  it('shows "+ Add time" link when post has no schedule', async () => {
    renderExpanded(makePost({ schedule: null }));
    expect(await screen.findByRole('button', { name: /\+ add time/i })).toBeInTheDocument();
    expect(screen.queryByLabelText(/scheduled time/i)).not.toBeInTheDocument();
  });

  it('clicking "+ Add time" reveals the datetime-local input', async () => {
    renderExpanded(makePost({ schedule: null }));
    fireEvent.click(await screen.findByRole('button', { name: /\+ add time/i }));
    expect(screen.getByLabelText(/scheduled time/i)).toBeInTheDocument();
  });

  it('shows a Clear button beside the input when schedule is set', async () => {
    renderExpanded(makePost({ schedule: '2026-06-01T10:00:00Z' }));
    await screen.findByLabelText(/scheduled time/i);
    expect(screen.getByRole('button', { name: /clear schedule/i })).toBeInTheDocument();
  });
});

describe('PostCard — schedule row — interactions (§17.5)', () => {
  it('changing the input calls update_post_schedule with UTC ISO', async () => {
    renderExpanded(makePost({ schedule: '2026-06-01T10:00:00Z' }));
    const input = await screen.findByLabelText(/scheduled time/i);
    fireEvent.change(input, { target: { value: '2026-06-01T14:00' } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_schedule', expect.objectContaining({
        repoPath: '/path/to/repo',
        postFolder: 'post-001',
        schedule: '2026-06-01T14:00:00.000Z',
      })),
    );
  });

  it('on update error: reverts input and shows error message', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_post_content') return '';
      if (cmd === 'get_attribution') return true;
      if (cmd === 'update_post_schedule') throw new Error('timestamp in the past');
      return null;
    });
    renderExpanded(makePost({ schedule: '2026-06-01T10:00:00Z' }));
    const input = await screen.findByLabelText(/scheduled time/i) as HTMLInputElement;
    fireEvent.change(input, { target: { value: '2026-06-01T14:00' } });
    await waitFor(() => {
      expect(screen.getByText(/timestamp in the past/i)).toBeInTheDocument();
      expect(input.value).toBe('2026-06-01T10:00');
    });
  });

  it('does not roll back when a newer change supersedes a failed one', async () => {
    let rejectFirst!: (_err: Error) => void;
    const firstCallHeld = new Promise<null>((_, reject) => { rejectFirst = reject; });
    let callCount = 0;
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_post_content') return '';
      if (cmd === 'get_attribution') return true;
      if (cmd === 'update_post_schedule') {
        callCount++;
        if (callCount === 1) return firstCallHeld;
        return null;
      }
      return null;
    });
    renderExpanded(makePost({ schedule: '2026-06-01T10:00:00Z' }));
    const input = await screen.findByLabelText(/scheduled time/i) as HTMLInputElement;

    // Two rapid changes; second resolves before first
    fireEvent.change(input, { target: { value: '2026-06-01T11:00' } });
    fireEvent.change(input, { target: { value: '2026-06-01T12:00' } });
    await waitFor(() => expect(callCount).toBe(2));

    // Fail the first (now stale) request
    rejectFirst(new Error('stale error'));

    // Input must stay at 12:00 with no error
    await waitFor(() => expect(input.value).toBe('2026-06-01T12:00'));
    expect(screen.queryByText(/stale error/i)).not.toBeInTheDocument();
  });
});

describe('PostCard — schedule row — clear (§17.5)', () => {
  it('Clear calls update_post_schedule with null and hides the input', async () => {
    mockConfirm.mockResolvedValue(true);
    renderExpanded(makePost({ schedule: '2026-06-01T10:00:00Z' }));
    await screen.findByLabelText(/scheduled time/i);
    fireEvent.click(screen.getByRole('button', { name: /clear schedule/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_schedule', expect.objectContaining({ schedule: null })),
    );
    expect(screen.queryByLabelText(/scheduled time/i)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /\+ add time/i })).toBeInTheDocument();
  });

  it('Clear does nothing when user cancels the confirmation', async () => {
    mockConfirm.mockResolvedValue(false);
    renderExpanded(makePost({ schedule: '2026-06-01T10:00:00Z' }));
    await screen.findByLabelText(/scheduled time/i);
    fireEvent.click(screen.getByRole('button', { name: /clear schedule/i }));
    await waitFor(() => expect(mockConfirm).toHaveBeenCalled());
    expect(mockInvoke).not.toHaveBeenCalledWith('update_post_schedule', expect.objectContaining({ schedule: null }));
    expect(screen.getByLabelText(/scheduled time/i)).toBeInTheDocument();
  });
});

describe('PostCard — schedule row — collapsed card reactivity (§17.5)', () => {
  it('collapsed card schedule text updates after a schedule change', async () => {
    render(
      <TimezoneContext.Provider value="UTC">
        <PostCard post={makePost({ schedule: '2026-06-01T10:00:00Z' })} onApproved={vi.fn()} onDismissed={vi.fn()} />
      </TimezoneContext.Provider>,
    );
    // Expand
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    const input = await screen.findByLabelText(/scheduled time/i);
    // Change schedule optimistically
    fireEvent.change(input, { target: { value: '2026-06-01T14:00' } });
    // Collapse
    fireEvent.click(screen.getByRole('button', { name: /preview/i }));
    // The collapsed card should now reflect the new schedule
    await waitFor(() => expect(screen.getByText(/2:00/)).toBeInTheDocument());
  });
});

describe('PostCard — collapsed card — timezone indicator (§fix-11)', () => {
  it('shows timezone offset label beside the formatted schedule', async () => {
    render(
      <TimezoneContext.Provider value="America/New_York">
        <PostCard post={makePost({ schedule: '2026-06-01T14:00:00Z' })} onApproved={vi.fn()} onDismissed={vi.fn()} />
      </TimezoneContext.Provider>,
    );
    // Collapsed card shows schedule + timezone label (getTimezoneOffsetLabel returns GMT±X)
    expect(await screen.findByText(/GMT[-+]/)).toBeInTheDocument();
  });

  it('shows no timezone label when post has no schedule', () => {
    render(
      <TimezoneContext.Provider value="UTC">
        <PostCard post={makePost({ schedule: null })} onApproved={vi.fn()} onDismissed={vi.fn()} />
      </TimezoneContext.Provider>,
    );
    expect(screen.queryByText(/GMT[+-]/)).not.toBeInTheDocument();
  });
});
