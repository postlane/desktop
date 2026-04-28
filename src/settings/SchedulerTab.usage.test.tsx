// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import SchedulerTab from './SchedulerTab';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

function setupMocks(usage: Record<string, { count: number; limit: number | null }> = {}) {
  mockInvoke.mockImplementation(async (cmd: unknown, args: unknown) => {
    if (cmd === 'get_scheduler_credential') throw new Error('not found');
    if (cmd === 'get_mastodon_connected_instance') return null;
    if (cmd === 'get_scheduler_usage') {
      const provider = (args as { provider: string }).provider;
      const u = usage[provider];
      if (u) return { provider, count: u.count, limit: u.limit, month: 4, year: 2026 };
      return { provider, count: 0, limit: null, month: 4, year: 2026 };
    }
    return null;
  });
}

describe('SchedulerTab — usage display (§13.1.3)', () => {
  it('shows post count for publer when posts have been made', async () => {
    setupMocks({ publer: { count: 7, limit: 10 } });
    render(<SchedulerTab />);
    await waitFor(() =>
      expect(screen.getByText(/7\/10 posts used this month/i)).toBeInTheDocument(),
    );
  });

  it('shows amber near-limit text at 80% of limit', async () => {
    setupMocks({ publer: { count: 8, limit: 10 } });
    render(<SchedulerTab />);
    await waitFor(() =>
      expect(screen.getByText(/approaching limit/i)).toBeInTheDocument(),
    );
  });

  it('shows limit-reached text and fallback notice at 100% of limit', async () => {
    setupMocks({ publer: { count: 10, limit: 10 } });
    render(<SchedulerTab />);
    await waitFor(() =>
      expect(screen.getByText(/limit reached/i)).toBeInTheDocument(),
    );
    expect(
      screen.getByText(/posts will fall back to your next configured provider/i),
    ).toBeInTheDocument();
  });

  it('shows post count for outstand', async () => {
    setupMocks({ outstand: { count: 500, limit: 1000 } });
    render(<SchedulerTab />);
    await waitFor(() =>
      expect(screen.getByText(/500\/1,000 posts used this month/i)).toBeInTheDocument(),
    );
  });

  it('does not show usage text for providers with no known limit', async () => {
    setupMocks({});
    render(<SchedulerTab />);
    // Wait for component to fully render
    await waitFor(() => screen.getByText(/zernio/i));
    expect(screen.queryByText(/posts used this month/i)).not.toBeInTheDocument();
  });
});
