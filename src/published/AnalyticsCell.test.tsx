// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { AnalyticsToggleCell } from './AnalyticsCell';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

describe('AnalyticsToggleCell', () => {
  it('renders a load trigger initially', () => {
    render(<AnalyticsToggleCell repoId="r1" postFolder="p1" />);
    expect(screen.getByRole('button', { name: /load analytics/i })).toBeInTheDocument();
  });

  it('calls get_post_analytics when triggered', async () => {
    mockInvoke.mockResolvedValue({ configured: false, sessions: 0, unique_sessions: 0, top_referrer: null });
    render(<AnalyticsToggleCell repoId="r1" postFolder="p1" />);
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('get_post_analytics', { repoId: 'r1', postFolder: 'p1' }),
    );
  });

  it('shows not-configured CTA when analytics.configured is false', async () => {
    mockInvoke.mockResolvedValue({ configured: false, sessions: 0, unique_sessions: 0, top_referrer: null });
    render(<AnalyticsToggleCell repoId="r1" postFolder="p1" />);
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() =>
      expect(screen.getByText(/set up analytics/i)).toBeInTheDocument(),
    );
  });

  it('shows session counts when traffic exists', async () => {
    mockInvoke.mockResolvedValue({ configured: true, sessions: 50, unique_sessions: 20, top_referrer: null });
    render(<AnalyticsToggleCell repoId="r1" postFolder="p1" />);
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() => expect(screen.getByText(/20 unique/)).toBeInTheDocument());
    expect(screen.getByText(/50 total/)).toBeInTheDocument();
  });

  it('shows "No sessions yet" for a recent post with zero sessions', async () => {
    const recentSentAt = new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString();
    mockInvoke.mockResolvedValue({ configured: true, sessions: 0, unique_sessions: 0, top_referrer: null });
    render(<AnalyticsToggleCell repoId="r1" postFolder="p1" sentAt={recentSentAt} />);
    fireEvent.click(screen.getByRole('button', { name: /load analytics/i }));
    await waitFor(() => expect(screen.getByText(/no sessions yet/i)).toBeInTheDocument());
  });
});
