// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, act } from '@testing-library/react';
import '@testing-library/jest-dom';
import userEvent from '@testing-library/user-event';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

import { invoke } from '../ipc/invoke';
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
    return Promise.resolve(undefined);
  });
  mockListen.mockResolvedValue(() => {});
});

describe('ModalAccount', () => {
  it('test_github_button_calls_openUrl', async () => {
    render(<ModalAccount onNext={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: /github/i }));
    expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/login?desktop=1&port=47312&provider=github');
  });

  it('test_gitlab_button_calls_openUrl', async () => {
    render(<ModalAccount onNext={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: /gitlab/i }));
    expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/login?desktop=1&port=47312&provider=gitlab');
  });

  it('test_no_next_button', () => {
    render(<ModalAccount onNext={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /next/i })).toBeNull();
  });

  it('test_no_manual_checkin_button', () => {
    render(<ModalAccount onNext={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /already signed in/i })).toBeNull();
  });

  it('test_license_activated_event_calls_onNext_with_provider', async () => {
    const onNext = vi.fn();
    render(<ModalAccount onNext={onNext} />);
    await userEvent.click(screen.getByRole('button', { name: /github/i }));
    // Effect re-runs after activeProvider is set — use the most recently registered listener
    await waitFor(() => {
      const entries = mockListen.mock.calls.filter(([ev]) => ev === 'license:activated');
      expect(entries.length).toBeGreaterThanOrEqual(2);
    });
    const entries = mockListen.mock.calls.filter(([ev]) => ev === 'license:activated');
    const latest = entries[entries.length - 1];
    act(() => (latest[1] as () => void)());
    expect(onNext).toHaveBeenCalledWith('github');
  });

  it('test_license_error_event_shows_message', async () => {
    render(<ModalAccount onNext={vi.fn()} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('license:error', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'license:error');
    if (!entry) throw new Error('license:error listener not registered');
    act(() => (entry[1] as (e: { payload: { message: string } }) => void)({ payload: { message: 'Token was rejected by the license server' } }));
    expect(screen.getByRole('alert')).toHaveTextContent('Token was rejected by the license server');
  });
});
