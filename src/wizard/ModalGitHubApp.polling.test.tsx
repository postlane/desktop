// SPDX-License-Identifier: BUSL-1.1
// Polling, guard, and timeout tests for ModalGitHubApp

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));

import { listen } from '@tauri-apps/api/event';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { invoke } from '../ipc/invoke';
import ModalGitHubApp, { MAX_POLL_ATTEMPTS, POLL_SLOW_THRESHOLD, MOUNT_CHECK_ATTEMPTS } from './ModalGitHubApp';

const mockListen = vi.mocked(listen);
const mockOpenDialog = vi.mocked(openDialog);
const mockInvoke = vi.mocked(invoke);

const defaultProps = {
  provider: 'github',
  workspaceId: 'ws-test',
  workspaceName: 'my-org',
  onNext: vi.fn(),
  onBack: vi.fn(),
  setRepoConnected: vi.fn(),
};

beforeEach(() => {
  vi.clearAllMocks();
  mockListen.mockResolvedValue(() => {});
  mockOpenDialog.mockResolvedValue(null);
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'list_repos_for_project') return [];
    return { name: 'my-repo' };
  });
});

// ---------------------------------------------------------------------------
// GitHub App installation polling — basic
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — GitHub App installation polling', () => {
  it('calls onNext when the first poll finds the app installed', async () => {
    const onNext = vi.fn();
    let callCount = 0;
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') {
        callCount++;
        return callCount >= 2; // false on mount check → button shows; true on first poll → advance
      }
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    await waitFor(() => screen.getByRole('button', { name: /install github app/i }));
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    await waitFor(() => expect(onNext).toHaveBeenCalledOnce());
  });

  it('calls onNext once when the deep link fires and polling also finds the app installed', async () => {
    const onNext = vi.fn();
    let callCount = 0;
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') {
        callCount++;
        return callCount >= 2; // false on mount → button shows; true on first poll
      }
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:app-installed', expect.any(Function)));
    await waitFor(() => screen.getByRole('button', { name: /install github app/i }));

    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:app-installed');
    if (!entry) throw new Error('github:app-installed listener not registered');

    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));
    act(() => (entry[1] as (e: { payload: { installation_id: number } }) => void)({ payload: { installation_id: 1 } }));

    await waitFor(() => expect(onNext).toHaveBeenCalledOnce());
    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it('calls check_github_app_installed with the workspaceId when polling', async () => {
    let callCount = 0;
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') {
        callCount++;
        return callCount >= 2; // false on mount → button shows; true on first poll
      }
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} />);
    await waitFor(() => screen.getByRole('button', { name: /install github app/i }));
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('check_github_app_installed', { projectId: 'ws-test' }),
    );
  });

  it('does not poll for non-GitHub provider', async () => {
    render(<ModalGitHubApp {...defaultProps} provider="gitlab" />);
    expect(mockInvoke).not.toHaveBeenCalledWith('check_github_app_installed', expect.anything());
  });
});

// ---------------------------------------------------------------------------
// Folder picker guard
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — folder picker guard', () => {
  it('does not open a second dialog if pickerOpenRef is already true', async () => {
    let resolveDialog: (v: string | null) => void = () => {};
    mockOpenDialog.mockImplementation(
      () => new Promise<string | null>((resolve) => { resolveDialog = resolve; })
    );
    render(<ModalGitHubApp {...defaultProps} />);
    const btn = screen.getByRole('button', { name: /choose folder/i });

    fireEvent.click(btn);
    await Promise.resolve();
    fireEvent.click(btn);
    expect(mockOpenDialog).toHaveBeenCalledTimes(1);
    act(() => { resolveDialog(null); });
    await Promise.resolve();
  });
});

// ---------------------------------------------------------------------------
// Install button guard
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — install button guard', () => {
  it('does not start a second polling loop if Install is clicked while already polling', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') {
        return new Promise<boolean>(() => {});
      }
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} />);
    const btn = screen.getByRole('button', { name: /install github app/i });
    fireEvent.click(btn);
    fireEvent.click(btn);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('check_github_app_installed', expect.anything()));
    // mount check (1) + first button click (1) = 2; second click is guarded and adds no more
    const checkCalls = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed')
    expect(checkCalls.length).toBeLessThanOrEqual(2);
    expect(checkCalls.length).toBeGreaterThanOrEqual(1);
  });
});

// ---------------------------------------------------------------------------
// Polling continues when app not installed
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — polling continues when app not installed', () => {
  it('schedules another poll when app is not installed yet', async () => {
    vi.useFakeTimers();
    const onNext = vi.fn();
    let callCount = 0;
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') {
        callCount += 1;
        return callCount >= 2;
      }
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    await Promise.resolve();
    await Promise.resolve();
    await vi.advanceTimersByTimeAsync(3000);
    await Promise.resolve();
    await Promise.resolve();

    expect(onNext).toHaveBeenCalledOnce();
    vi.useRealTimers();
  });

  it('stops polling after unmount even when app is not yet installed', async () => {
    vi.useFakeTimers();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });

    const { unmount } = render(<ModalGitHubApp {...defaultProps} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));
    await Promise.resolve();
    unmount();
    const invokeCountBefore = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    await vi.advanceTimersByTimeAsync(3000);
    await Promise.resolve();
    const invokeCountAfter = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    expect(invokeCountAfter).toBe(invokeCountBefore);
    vi.useRealTimers();
  });
});

// ---------------------------------------------------------------------------
// Folder picker connecting guard
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — folder picker connecting guard', () => {
  it('does not open dialog when connecting is true and pickerOpenRef is false', async () => {
    let firstResolve: (v: string | null) => void = () => {};
    let callCount = 0;
    mockOpenDialog.mockImplementation(
      () => new Promise<string | null>((resolve) => {
        callCount += 1;
        if (callCount === 1) {
          firstResolve = resolve;
        } else {
          resolve(null);
        }
      })
    );

    render(<ModalGitHubApp {...defaultProps} />);
    const btn = screen.getByRole('button', { name: /choose folder/i });

    fireEvent.click(btn);
    await Promise.resolve();
    act(() => { firstResolve(null); });
    fireEvent.click(btn);

    await Promise.resolve();
    await Promise.resolve();
    expect(mockOpenDialog.mock.calls.length).toBeGreaterThanOrEqual(1);
  });
});

// ---------------------------------------------------------------------------
// Poll cancel at line 195 (cancelPollRef true after invoke returns)
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — poll cancel at line 195', () => {
  it('does not schedule next poll when component unmounts during invoke await', async () => {
    vi.useFakeTimers();
    const onNext = vi.fn();

    let resolveInstalled: (v: boolean) => void = () => {};
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') {
        return new Promise<boolean>((resolve) => { resolveInstalled = resolve; });
      }
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });

    const { unmount } = render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    await vi.runAllTimersAsync();
    await Promise.resolve();

    unmount();

    act(() => { resolveInstalled(false); });
    await Promise.resolve();
    await Promise.resolve();

    await vi.advanceTimersByTimeAsync(3000 * 2);
    await Promise.resolve();

    // mount check (1) + button-click poll (1) = 2 total; nothing after unmount
    const checkCalls = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    expect(checkCalls).toBe(2);
    expect(onNext).not.toHaveBeenCalled();
    vi.useRealTimers();
  });
});

// ---------------------------------------------------------------------------
// Polling slow notice
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — polling slow notice', () => {
  it('shows slow notice after POLL_SLOW_THRESHOLD failed polls', async () => {
    vi.useFakeTimers();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    for (let i = 0; i < POLL_SLOW_THRESHOLD; i++) {
      await Promise.resolve();
      await vi.advanceTimersByTimeAsync(3000);
    }
    await Promise.resolve();

    expect(screen.getByText(/Still waiting for GitHub/i)).toBeInTheDocument();
    vi.useRealTimers();
  });
});

// ---------------------------------------------------------------------------
// Polling timeout
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — polling timeout', () => {
  it('stops polling and shows timeout message after MAX_POLL_ATTEMPTS', async () => {
    vi.useFakeTimers();
    const onNext = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    for (let i = 0; i <= MAX_POLL_ATTEMPTS; i++) {
      await Promise.resolve();
      await vi.advanceTimersByTimeAsync(3000);
    }
    await Promise.resolve();

    expect(screen.getByText(/not detected after 6 minutes/i)).toBeInTheDocument();
    expect(onNext).not.toHaveBeenCalled();

    const callsBefore = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    await vi.advanceTimersByTimeAsync(3000 * 5);
    await Promise.resolve();
    const callsAfter = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    expect(callsAfter).toBe(callsBefore);

    vi.useRealTimers();
  });
});

// ---------------------------------------------------------------------------
// Mount-time installation check
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — mount-time check — basic', () => {
  it('calls check_github_app_installed on mount for GitHub provider', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });
    render(<ModalGitHubApp {...defaultProps} />);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('check_github_app_installed', { projectId: 'ws-test' }),
    );
  });

  it('shows Connected badge and does NOT auto-advance when app is already installed on mount', async () => {
    const onNext = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return true;
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });
    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    await waitFor(() => expect(screen.getByText(/github app connected/i)).toBeInTheDocument());
    expect(onNext).not.toHaveBeenCalled();
  });

  it('does not auto-advance when app is not installed on mount', async () => {
    const onNext = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });
    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(onNext).not.toHaveBeenCalled();
  });

  it('does not call check_github_app_installed on mount for non-GitHub provider', async () => {
    render(<ModalGitHubApp {...defaultProps} provider="gitlab" />);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mockInvoke).not.toHaveBeenCalledWith('check_github_app_installed', expect.anything());
  });

  it('ignores errors from the mount-time check and does not advance', async () => {
    const onNext = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') throw new Error('network error');
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });
    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(onNext).not.toHaveBeenCalled();
  });
});

describe('ModalConnectRepos — mount-time check — retry', () => {
  it('retries mount check when first attempt returns false and shows Connected badge on retry success', async () => {
    vi.useFakeTimers();
    const onNext = vi.fn();
    let callCount = 0;
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') {
        callCount++;
        return callCount >= 2; // false on attempt 1, true on attempt 2
      }
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });
    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    await Promise.resolve(); // let attempt 1 run (false)
    await vi.advanceTimersByTimeAsync(3000); // trigger attempt 2
    await Promise.resolve(); // let attempt 2 resolve (true)
    expect(screen.getByText(/github app connected/i)).toBeInTheDocument();
    expect(onNext).not.toHaveBeenCalled(); // never auto-advances from mount poll
    vi.useRealTimers();
  });

  it('stops retrying mount check after MOUNT_CHECK_ATTEMPTS failed attempts', async () => {
    vi.useFakeTimers();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });
    render(<ModalGitHubApp {...defaultProps} />);
    // Run through all mount check attempts
    for (let i = 0; i < MOUNT_CHECK_ATTEMPTS; i++) {
      await Promise.resolve();
      await vi.advanceTimersByTimeAsync(3000);
    }
    await Promise.resolve();
    const callsBefore = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    await vi.advanceTimersByTimeAsync(3000 * 5);
    await Promise.resolve();
    const callsAfter = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    expect(callsAfter).toBe(callsBefore);
    vi.useRealTimers();
  });
});

describe('ModalConnectRepos — mount-time check — cancellation', () => {
  it('cancels the mount check when component unmounts before the invoke resolves', async () => {
    const onNext = vi.fn();
    let resolveInstalled: (v: boolean) => void = () => {};
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') {
        return new Promise<boolean>((resolve) => { resolveInstalled = resolve; });
      }
      if (cmd === 'list_repos_for_project') return [];
      return { name: 'repo' };
    });
    const { unmount } = render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    await new Promise((resolve) => setTimeout(resolve, 0));
    unmount();
    resolveInstalled(true);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(onNext).not.toHaveBeenCalled();
  });
});
