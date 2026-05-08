// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));

import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
const mockInvoke = vi.mocked(invoke);
const mockOpenUrl = vi.mocked(openUrl);

import ReSignInScreen from './ReSignInScreen';

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockImplementation((cmd: unknown) => {
    if (cmd === 'get_local_server_port') return Promise.resolve(47312);
    return Promise.resolve(false);
  });
});

describe('ReSignInScreen', () => {
  it('test_renders_sign_in_message', () => {
    render(<ReSignInScreen onSignedIn={vi.fn()} pollIntervalMs={10000} />);
    expect(screen.getByText(/session has expired/i)).toBeDefined();
  });

  it('test_calls_onSignedIn_when_token_detected', async () => {
    mockInvoke.mockResolvedValue(true);
    const onSignedIn = vi.fn();
    render(<ReSignInScreen onSignedIn={onSignedIn} pollIntervalMs={30} />);
    await waitFor(() => expect(onSignedIn).toHaveBeenCalledOnce(), { timeout: 3000 });
  });

  it('github button opens login URL with desktop=1 and port', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'get_local_server_port') return Promise.resolve(47312);
      return Promise.resolve(false);
    });
    render(<ReSignInScreen onSignedIn={vi.fn()} pollIntervalMs={10000} />);
    await userEvent.click(screen.getByRole('button', { name: /github/i }));
    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalled());
    const url = mockOpenUrl.mock.calls[0][0] as string;
    expect(url).toContain('desktop=1');
    expect(url).toContain('port=47312');
    expect(url).toContain('provider=github');
  });

  it('gitlab button opens login URL with desktop=1, port, and provider=gitlab', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'get_local_server_port') return Promise.resolve(47312);
      return Promise.resolve(false);
    });
    render(<ReSignInScreen onSignedIn={vi.fn()} pollIntervalMs={10000} />);
    await userEvent.click(screen.getByRole('button', { name: /gitlab/i }));
    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalled());
    const url = mockOpenUrl.mock.calls[0][0] as string;
    expect(url).toContain('desktop=1');
    expect(url).toContain('port=47312');
    expect(url).toContain('provider=gitlab');
  });

  it('falls back to desktop=1 without port when get_local_server_port fails', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'get_local_server_port') return Promise.reject(new Error('no port file'));
      return Promise.resolve(false);
    });
    render(<ReSignInScreen onSignedIn={vi.fn()} pollIntervalMs={10000} />);
    await userEvent.click(screen.getByRole('button', { name: /github/i }));
    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalled());
    const url = mockOpenUrl.mock.calls[0][0] as string;
    expect(url).toContain('desktop=1');
    expect(url).not.toContain('port=');
    expect(url).toContain('provider=github');
  });
});
