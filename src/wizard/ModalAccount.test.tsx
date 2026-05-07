// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, act } from '@testing-library/react';
import '@testing-library/jest-dom';
import userEvent from '@testing-library/user-event';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { listen } from '@tauri-apps/api/event';
const mockInvoke = vi.mocked(invoke);
const mockOpenUrl = vi.mocked(openUrl);
const mockListen = vi.mocked(listen);

import ModalAccount from './ModalAccount';

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_local_server_port') return Promise.resolve(47312);
    return Promise.resolve(false);
  });
  mockListen.mockResolvedValue(() => {});
});

describe('ModalAccount', () => {
  it('test_github_button_calls_openUrl', async () => {
    render(<ModalAccount onNext={vi.fn()} pollIntervalMs={10000} />);
    await userEvent.click(screen.getByRole('button', { name: /github/i }));
    expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/login?desktop=1&port=47312');
  });

  it('test_gitlab_button_calls_openUrl', async () => {
    render(<ModalAccount onNext={vi.fn()} pollIntervalMs={10000} />);
    await userEvent.click(screen.getByRole('button', { name: /gitlab/i }));
    expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/login?desktop=1&port=47312');
  });

  it('test_auto_advances_when_token_detected', async () => {
    mockInvoke.mockResolvedValue(true);
    const onNext = vi.fn();
    render(<ModalAccount onNext={onNext} pollIntervalMs={30} />);
    await waitFor(() => expect(onNext).toHaveBeenCalledOnce(), { timeout: 3000 });
  });

  it('test_poll_clears_on_unmount', () => {
    const clearSpy = vi.spyOn(globalThis, 'clearInterval');
    const { unmount } = render(<ModalAccount onNext={vi.fn()} pollIntervalMs={10000} />);
    unmount();
    expect(clearSpy).toHaveBeenCalled();
    clearSpy.mockRestore();
  });

  it('test_no_next_button', () => {
    render(<ModalAccount onNext={vi.fn()} pollIntervalMs={10000} />);
    expect(screen.queryByRole('button', { name: /next/i })).toBeNull();
  });

  it('test_no_manual_checkin_button', () => {
    render(<ModalAccount onNext={vi.fn()} pollIntervalMs={10000} />);
    expect(screen.queryByRole('button', { name: /already signed in/i })).toBeNull();
  });

  it('test_license_activated_event_calls_onNext', async () => {
    const onNext = vi.fn();
    render(<ModalAccount onNext={onNext} pollIntervalMs={10000} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('license:activated', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'license:activated');
    if (!entry) throw new Error('license:activated listener not registered');
    act(() => (entry[1] as () => void)());
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('test_license_error_event_shows_message', async () => {
    render(<ModalAccount onNext={vi.fn()} pollIntervalMs={10000} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('license:error', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'license:error');
    if (!entry) throw new Error('license:error listener not registered');
    act(() => (entry[1] as (e: { payload: { message: string } }) => void)({ payload: { message: 'Token was rejected by the license server' } }));
    expect(screen.getByRole('alert')).toHaveTextContent('Token was rejected by the license server');
  });
});
