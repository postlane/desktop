// SPDX-License-Identifier: BUSL-1.1
// Tests for the cancel error filter in ScheduledRow.
// Kept in a separate file because RepoPublishedView.test.tsx is at the 400-line limit.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import RepoPublishedView from './RepoPublishedView';
import type { PublishedPost } from '../types';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));

import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

function makeQueued(overrides: Partial<PublishedPost> = {}): PublishedPost {
  return {
    repo_id: 'r1',
    repo_name: 'my-app',
    repo_path: '/path/to/repo',
    post_folder: 'post-001',
    status: 'queued',
    platforms: ['x'],
    platform_results: null,
    schedule: '2026-07-01T12:00:00Z',
    scheduler_ids: { x: 'sched-123' },
    platform_urls: null,
    llm_model: null,
    provider: null,
    sent_at: null,
    created_at: null,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Cancel error filter — "not yet available" stub messages (Fix 5)
// ---------------------------------------------------------------------------

describe('RepoPublishedView — cancel error filter (not yet available)', () => {
  it('maps "not yet available" error to "Cancel via dashboard"', async () => {
    mockInvoke
      .mockResolvedValueOnce([makeQueued({ post_folder: 'q1', scheduler_ids: { x: 'id-99' } })])
      .mockRejectedValueOnce(new Error('Post cancellation is not yet available — please delete the draft instead.'));
    render(<RepoPublishedView repoId="r1" />);
    await waitFor(() => screen.getByRole('button', { name: /cancel/i }));
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    await waitFor(() =>
      expect(screen.getByText(/cancel via dashboard/i)).toBeInTheDocument(),
    );
  });
});
