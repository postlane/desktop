// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import AnalyticsTab from './AnalyticsTab';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

describe('AnalyticsTab — not signed in', () => {
  it('shows sign-in prompt when get_site_token fails', async () => {
    mockInvoke.mockRejectedValue(new Error('Not signed in'));
    render(<AnalyticsTab repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByText(/sign in/i)).toBeInTheDocument(),
    );
  });

  it('shows a link to postlane.dev when not signed in', async () => {
    mockInvoke.mockRejectedValue(new Error('Not signed in'));
    render(<AnalyticsTab repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByRole('link', { name: /postlane\.dev/i })).toBeInTheDocument(),
    );
  });
});

describe('AnalyticsTab — configured', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_site_token') return 'tok-abc123';
      return null;
    });
  });

  it('shows the script tag with the site token', async () => {
    render(<AnalyticsTab repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByText(/tok-abc123/)).toBeInTheDocument(),
    );
  });

  it('shows a Copy button', async () => {
    render(<AnalyticsTab repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /copy/i })).toBeInTheDocument(),
    );
  });

  it('shows the head instruction', async () => {
    render(<AnalyticsTab repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByText(/add this tag/i)).toBeInTheDocument(),
    );
  });
});

describe('AnalyticsTab — no repoId', () => {
  it('shows a prompt to select a repo when repoId is null', () => {
    render(<AnalyticsTab repoId={null} />);
    expect(screen.getByText(/select a repo/i)).toBeInTheDocument();
  });
});

describe('AnalyticsTab — copy error', () => {
  it('shows error when clipboard write fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_site_token') return 'tok-abc123';
      return null;
    });
    const writeText = vi.fn().mockRejectedValue(new Error('Permission denied'));
    vi.stubGlobal('navigator', { ...globalThis.navigator, clipboard: { writeText } });
    render(<AnalyticsTab repoId="r1" />);
    await waitFor(() => screen.getByRole('button', { name: /copy/i }));
    fireEvent.click(screen.getByRole('button', { name: /copy/i }));
    await waitFor(() => expect(screen.getByText(/failed to copy/i)).toBeInTheDocument());
    vi.unstubAllGlobals();
  });
});
