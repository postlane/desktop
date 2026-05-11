// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';

const mockInvoke = vi.fn();
const mockListen = vi.fn();
const mockOpenUrl = vi.fn();

vi.mock('../ipc/invoke', () => ({ invoke: (...a: unknown[]) => mockInvoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a: unknown[]) => mockListen(...a) }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: (...a: unknown[]) => mockOpenUrl(...a) }));

import { LicenseSection } from './LicenseSection';

beforeEach(() => {
  vi.clearAllMocks();
  mockListen.mockResolvedValue(() => {});
  mockOpenUrl.mockResolvedValue(undefined);
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_local_server_port') return Promise.resolve(47312);
    return Promise.resolve(false);
  });
});

describe('LicenseSection — sign-in state', () => {
  it('shows the sign-in button when not signed in', async () => {
    mockInvoke.mockResolvedValue(false);
    render(<LicenseSection />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /sign in at postlane\.dev/i })).toBeInTheDocument(),
    );
  });

  it('hides the sign-in button when already signed in', async () => {
    mockInvoke.mockResolvedValue(true);
    render(<LicenseSection />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_license_signed_in'));
    expect(screen.queryByRole('button', { name: /sign in/i })).not.toBeInTheDocument();
  });

  it('opens login URL with desktop=1 and port when the button is clicked', async () => {
    render(<LicenseSection />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /sign in at postlane\.dev/i })).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole('button', { name: /sign in at postlane\.dev/i }));
    await waitFor(() =>
      expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/login?desktop=1&port=47312'),
    );
  });

  it('falls back to login without port when get_local_server_port fails', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_local_server_port') return Promise.reject(new Error('port not available'));
      return Promise.resolve(false);
    });
    render(<LicenseSection />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /sign in at postlane\.dev/i })).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole('button', { name: /sign in at postlane\.dev/i }));
    await waitFor(() =>
      expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/login?desktop=1'),
    );
  });
});

type ActivatedHandler = (e: { payload: { display_name: string } }) => void;

function captureActivatedHandler(): { handler: ActivatedHandler | null } {
  const captured: { handler: ActivatedHandler | null } = { handler: null };
  mockListen.mockImplementation((event: string, handler: ActivatedHandler) => {
    if (event === 'license:activated') captured.handler = handler;
    return Promise.resolve(() => {});
  });
  return captured;
}

describe('LicenseSection — activation events', () => {
  it('shows the activation confirmation banner when license:activated event fires', async () => {
    mockInvoke.mockResolvedValue(false);
    const captured = captureActivatedHandler();
    render(<LicenseSection />);
    await waitFor(() => expect(captured.handler).not.toBeNull());
    if (captured.handler === null) throw new Error('license:activated handler was not registered');
    captured.handler({ payload: { display_name: 'Ada Lovelace' } });
    await waitFor(() =>
      expect(screen.getByText(/postlane activated.*ada lovelace/i)).toBeInTheDocument(),
    );
  });

  it('hides the sign-in button after activation', async () => {
    mockInvoke.mockResolvedValue(false);
    const captured = captureActivatedHandler();
    render(<LicenseSection />);
    await waitFor(() => expect(captured.handler).not.toBeNull());
    if (captured.handler === null) throw new Error('license:activated handler was not registered');
    captured.handler({ payload: { display_name: 'Ada' } });
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: /sign in/i })).not.toBeInTheDocument(),
    );
  });
});

function captureExpiredHandler(): { handler: (() => void) | null } {
  const captured: { handler: (() => void) | null } = { handler: null };
  mockListen.mockImplementation((event: string, handler: () => void) => {
    if (event === 'license:expired') captured.handler = handler;
    return Promise.resolve(() => {});
  });
  return captured;
}

describe('LicenseSection — expiry events', () => {
  it('shows expired banner when license:expired event fires', async () => {
    mockInvoke.mockResolvedValue(true);
    const captured = captureExpiredHandler();
    render(<LicenseSection />);
    await waitFor(() => expect(captured.handler).not.toBeNull());
    if (captured.handler === null) throw new Error('license:expired handler was not registered');
    captured.handler();
    await waitFor(() =>
      expect(screen.getByText(/your postlane license has expired/i)).toBeInTheDocument(),
    );
  });
});
