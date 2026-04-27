// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';

const mockInvoke = vi.fn();
const mockListen = vi.fn();
const mockOpenUrl = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => mockInvoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a: unknown[]) => mockListen(...a) }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: (...a: unknown[]) => mockOpenUrl(...a) }));

import { LicenseSection } from './LicenseSection';

beforeEach(() => {
  vi.clearAllMocks();
  mockListen.mockResolvedValue(() => {});
  mockOpenUrl.mockResolvedValue(undefined);
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

  it('opens https://postlane.dev/login in the browser when the button is clicked', async () => {
    mockInvoke.mockResolvedValue(false);
    render(<LicenseSection />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /sign in at postlane\.dev/i })).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole('button', { name: /sign in at postlane\.dev/i }));
    await waitFor(() =>
      expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/login'),
    );
  });
});

describe('LicenseSection — activation events', () => {
  type ActivatedHandler = (e: { payload: { display_name: string } }) => void;

  it('shows the activation confirmation banner when license:activated event fires', async () => {
    mockInvoke.mockResolvedValue(false);
    let activatedHandler: ActivatedHandler | null = null;
    mockListen.mockImplementation((event: string, handler: ActivatedHandler) => {
      if (event === 'license:activated') activatedHandler = handler;
      return Promise.resolve(() => {});
    });
    render(<LicenseSection />);
    await waitFor(() => expect(activatedHandler).not.toBeNull());
    if (activatedHandler === null) throw new Error('license:activated handler was not registered');
    activatedHandler({ payload: { display_name: 'Ada Lovelace' } });
    await waitFor(() =>
      expect(screen.getByText(/postlane activated.*ada lovelace/i)).toBeInTheDocument(),
    );
  });

  it('hides the sign-in button after activation', async () => {
    mockInvoke.mockResolvedValue(false);
    let activatedHandler: ActivatedHandler | null = null;
    mockListen.mockImplementation((event: string, handler: ActivatedHandler) => {
      if (event === 'license:activated') activatedHandler = handler;
      return Promise.resolve(() => {});
    });
    render(<LicenseSection />);
    await waitFor(() => expect(activatedHandler).not.toBeNull());
    if (activatedHandler === null) throw new Error('license:activated handler was not registered');
    activatedHandler({ payload: { display_name: 'Ada' } });
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: /sign in/i })).not.toBeInTheDocument(),
    );
  });
});

describe('LicenseSection — expiry events', () => {
  it('shows expired banner when license:expired event fires', async () => {
    mockInvoke.mockResolvedValue(true);
    let expiredHandler: (() => void) | null = null;
    mockListen.mockImplementation((event: string, handler: () => void) => {
      if (event === 'license:expired') expiredHandler = handler;
      return Promise.resolve(() => {});
    });
    render(<LicenseSection />);
    await waitFor(() => expect(expiredHandler).not.toBeNull());
    if (expiredHandler === null) throw new Error('license:expired handler was not registered');
    expiredHandler();
    await waitFor(() =>
      expect(screen.getByText(/your postlane license has expired/i)).toBeInTheDocument(),
    );
  });
});
