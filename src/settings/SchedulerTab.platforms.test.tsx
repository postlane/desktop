// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import SchedulerTab from './SchedulerTab';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));

import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'get_scheduler_credential') throw new Error('not found');
    if (cmd === 'get_mastodon_connected_instance') return null;
    return null;
  });
});

describe('SchedulerTab — platform coverage tags (§17.2)', () => {
  it('shows Bluesky badge on the Zernio card', async () => {
    render(<SchedulerTab />);
    const card = (await screen.findByText(/zernio/i)).closest('[data-provider]');
    expect(card).not.toBeNull();
    expect(card).toHaveTextContent('Bluesky');
  });

  it('shows Mastodon badge on the Zernio card', async () => {
    render(<SchedulerTab />);
    const card = (await screen.findByText(/zernio/i)).closest('[data-provider]');
    expect(card).toHaveTextContent('Mastodon');
  });

  it('does not show Bluesky badge on the Publer card', async () => {
    render(<SchedulerTab />);
    const card = (await screen.findByText(/publer/i)).closest('[data-provider]');
    expect(card).not.toHaveTextContent('Bluesky');
  });

  it('shows TikTok badge on the Publer card', async () => {
    render(<SchedulerTab />);
    const card = (await screen.findByText(/publer/i)).closest('[data-provider]');
    expect(card).toHaveTextContent('TikTok');
  });

  it('does not show TikTok badge on the Outstand card', async () => {
    render(<SchedulerTab />);
    const card = (await screen.findByText(/outstand/i)).closest('[data-provider]');
    expect(card).not.toHaveTextContent('TikTok');
  });

  it('does not render a card for the removed "ayrshare" provider', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    expect(screen.queryByText(/ayrshare/i)).not.toBeInTheDocument();
  });

  it('does not render a card for the removed "buffer" provider', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    expect(screen.queryByText(/buffer/i)).not.toBeInTheDocument();
  });
});
