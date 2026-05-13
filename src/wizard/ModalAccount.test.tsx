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

describe('ModalAccount — mode prop', () => {
  it('test_sign_in_mode_shows_sign_in_title', () => {
    render(<ModalAccount onNext={vi.fn()} mode="sign_in" />);
    expect(screen.getByRole('heading', { name: /sign in to postlane/i })).toBeInTheDocument();
  });

  it('test_add_org_mode_shows_add_org_title', () => {
    render(<ModalAccount onNext={vi.fn()} mode="add_org" />);
    expect(screen.getByRole('heading', { name: /add an organization/i })).toBeInTheDocument();
  });

  it('test_add_org_mode_shows_add_org_subtitle', () => {
    render(<ModalAccount onNext={vi.fn()} mode="add_org" />);
    expect(screen.getByText(/choose the provider where your org is hosted/i)).toBeInTheDocument();
  });

  it('test_default_mode_shows_sign_in_title', () => {
    render(<ModalAccount onNext={vi.fn()} />);
    expect(screen.getByRole('heading', { name: /sign in to postlane/i })).toBeInTheDocument();
  });
});

describe('ModalAccount — OAuth buttons', () => {
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
});

describe('ModalAccount — events and links', () => {
  it('test_license_error_event_shows_message', async () => {
    render(<ModalAccount onNext={vi.fn()} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('license:error', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'license:error');
    if (!entry) throw new Error('license:error listener not registered');
    act(() => (entry[1] as (e: { payload: { message: string } }) => void)({ payload: { message: 'Token was rejected by the license server' } }));
    expect(screen.getByRole('alert')).toHaveTextContent('Token was rejected by the license server');
  });

  it('test_github_button_opens_url_without_port_when_invoke_fails', async () => {
    mockInvoke.mockRejectedValue(new Error('server not started'));
    render(<ModalAccount onNext={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: /github/i }));
    await waitFor(() =>
      expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/login?desktop=1&provider=github'),
    );
  });

  it('test_gitlab_button_opens_url_without_port_when_invoke_fails', async () => {
    mockInvoke.mockRejectedValue(new Error('server not started'));
    render(<ModalAccount onNext={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: /gitlab/i }));
    await waitFor(() =>
      expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/login?desktop=1&provider=gitlab'),
    );
  });

  it('test_privacy_link_calls_openUrl', async () => {
    render(<ModalAccount onNext={vi.fn()} />);
    await userEvent.click(screen.getByRole('link', { name: /privacy page/i }));
    expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/privacy');
  });

  it('test_security_link_calls_openUrl', async () => {
    render(<ModalAccount onNext={vi.fn()} />);
    await userEvent.click(screen.getByRole('link', { name: /security docs/i }));
    expect(mockOpenUrl).toHaveBeenCalledWith('https://docs.postlane.dev/security');
  });
});
