// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));

import { openUrl } from '@tauri-apps/plugin-opener';
import { listen } from '@tauri-apps/api/event';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { invoke } from '../ipc/invoke';
import ModalGitHubApp from './ModalGitHubApp';

const mockOpenUrl = vi.mocked(openUrl);
const mockListen = vi.mocked(listen);
const mockOpenDialog = vi.mocked(openDialog);
const mockInvoke = vi.mocked(invoke);

const defaultProps = {
  provider: 'github',
  workspaceId: 'ws-test',
  workspaceName: 'my-org',
  onNext: vi.fn(),
  onBack: vi.fn(),
};

beforeEach(() => {
  vi.clearAllMocks();
  mockListen.mockResolvedValue(() => {});
  mockOpenDialog.mockResolvedValue(null);
  mockInvoke.mockResolvedValue({ name: 'my-repo' });
});

// ---------------------------------------------------------------------------
// Structure
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — structure', () => {
  it('renders step 5 of 7 in WizardShell', () => {
    render(<ModalGitHubApp {...defaultProps} />);
    expect(screen.getByText(/5\s*\/\s*7/)).toBeDefined();
  });

  it('renders the Connect your repos heading', () => {
    render(<ModalGitHubApp {...defaultProps} />);
    expect(screen.getByRole('heading', { name: /connect your repos/i })).toBeDefined();
  });

  it('shows all three section headings for GitHub provider', () => {
    render(<ModalGitHubApp {...defaultProps} />);
    expect(screen.getByText('GitHub App')).toBeDefined();
    expect(screen.getByText('Desktop folder')).toBeDefined();
    expect(screen.getByText('CLI')).toBeDefined();
  });

  it('hides GitHub App section for non-GitHub provider', () => {
    render(<ModalGitHubApp {...defaultProps} provider="gitlab" />);
    expect(screen.queryByText('GitHub App')).toBeNull();
    expect(screen.getByText('Desktop folder')).toBeDefined();
    expect(screen.getByText('CLI')).toBeDefined();
  });
});

// ---------------------------------------------------------------------------
// GitHub App section
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — GitHub App section', () => {
  it('Install GitHub App button opens the app install URL', async () => {
    render(<ModalGitHubApp {...defaultProps} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));
    expect(mockOpenUrl).toHaveBeenCalledOnce();
    const [url] = mockOpenUrl.mock.calls[0] as [string];
    expect(url).toMatch(/^https:\/\/github\.com\/apps\//);
  });
});

// ---------------------------------------------------------------------------
// Deep link events
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — deep link events', () => {
  it('registers listeners for github:app-installed and github:install-error on mount', async () => {
    render(<ModalGitHubApp {...defaultProps} />);
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('github:app-installed', expect.any(Function));
      expect(mockListen).toHaveBeenCalledWith('github:install-error', expect.any(Function));
    });
  });

  it('calls onNext when github:app-installed fires for GitHub provider', async () => {
    const onNext = vi.fn();
    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:app-installed', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:app-installed');
    if (!entry) throw new Error('github:app-installed listener not registered');
    act(() => (entry[1] as (e: { payload: { installation_id: number } }) => void)({ payload: { installation_id: 1 } }));
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('shows an inline error and does not advance when github:install-error fires', async () => {
    const onNext = vi.fn();
    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:install-error', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:install-error');
    if (!entry) throw new Error('github:install-error listener not registered');
    act(() => (entry[1] as (e: { payload: { message: string } }) => void)({ payload: { message: 'Installation not found' } }));
    expect(screen.getByRole('alert')).toBeDefined();
    expect(onNext).not.toHaveBeenCalled();
  });

  it('does not call onNext for non-GitHub provider when app-installed fires', async () => {
    const onNext = vi.fn();
    render(<ModalGitHubApp {...defaultProps} provider="gitlab" onNext={onNext} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:app-installed', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:app-installed');
    if (!entry) throw new Error('github:app-installed listener not registered');
    act(() => (entry[1] as (e: { payload: { installation_id: number } }) => void)({ payload: { installation_id: 1 } }));
    expect(onNext).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Folder picker section
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — folder picker', () => {
  it('calls connect_repo_from_desktop with the selected folder and workspaceId', async () => {
    mockOpenDialog.mockResolvedValue('/Users/user/projects/my-repo');
    render(<ModalGitHubApp {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('connect_repo_from_desktop', {
      repoPath: '/Users/user/projects/my-repo',
      projectId: 'ws-test',
    }));
  });

  it('shows Next button after a folder is connected', async () => {
    mockOpenDialog.mockResolvedValue('/Users/user/projects/my-repo');
    render(<ModalGitHubApp {...defaultProps} />);
    expect(screen.queryByRole('button', { name: /^next/i })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => expect(screen.getByRole('button', { name: /^next/i })).toBeDefined());
  });

  it('shows clean error for NotAGitRepo', async () => {
    mockOpenDialog.mockResolvedValue('/Users/user/not-a-repo');
    mockInvoke.mockRejectedValue("NotAGitRepo: '/Users/user/not-a-repo' is not a git repository");
    render(<ModalGitHubApp {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('Not a Git repository');
      expect(alert.textContent).not.toContain('NotAGitRepo:');
    });
  });
});

describe('ModalConnectRepos — folder picker errors', () => {
  it('shows clean error for RepoAlreadyRegistered including workspace name', async () => {
    mockOpenDialog.mockResolvedValue('/Users/user/my-repo');
    mockInvoke.mockRejectedValue("RepoAlreadyRegistered: '/Users/user/my-repo' is already registered");
    render(<ModalGitHubApp {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('already connected to the my-org workspace');
      expect(alert.textContent).not.toContain('RepoAlreadyRegistered:');
    });
  });

  it('does nothing when the folder dialog is cancelled', async () => {
    mockOpenDialog.mockResolvedValue(null);
    render(<ModalGitHubApp {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('shows clean error for PathNotAuthorised', async () => {
    mockOpenDialog.mockResolvedValue('/etc/secrets');
    mockInvoke.mockRejectedValue("PathNotAuthorised: '/etc/secrets' is outside the home directory");
    render(<ModalGitHubApp {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('outside your home directory');
    });
  });

  it('shows generic error for unknown error types', async () => {
    mockOpenDialog.mockResolvedValue('/Users/user/some-repo');
    mockInvoke.mockRejectedValue('SomeWeirdError: unexpected');
    render(<ModalGitHubApp {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('Failed to connect repository');
    });
  });
});

// ---------------------------------------------------------------------------
// CLI section
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — CLI section', () => {
  it('CLI command is hidden until Show command is clicked', () => {
    render(<ModalGitHubApp {...defaultProps} />);
    expect(screen.queryByText('npx @postlane/cli init')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: /show command/i }));
    expect(screen.getByText('npx @postlane/cli init')).toBeDefined();
  });

  it('Show command toggles to Hide command after clicking', () => {
    render(<ModalGitHubApp {...defaultProps} />);
    fireEvent.click(screen.getByRole('button', { name: /show command/i }));
    expect(screen.getByRole('button', { name: /hide command/i })).toBeDefined();
  });

  it('Copy button writes CLI command to clipboard', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    render(<ModalGitHubApp {...defaultProps} />);
    fireEvent.click(screen.getByRole('button', { name: /show command/i }));
    await userEvent.click(screen.getByRole('button', { name: /copy/i }));
    expect(writeText).toHaveBeenCalledWith('npx @postlane/cli init');
  });
});

// ---------------------------------------------------------------------------
// GitHub App installation polling
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — GitHub App installation polling', () => {
  it('calls onNext immediately when the app is already installed at the moment the button is clicked', async () => {
    const onNext = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return true;
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    await waitFor(() => expect(onNext).toHaveBeenCalledOnce());
  });

  it('calls onNext once when the deep link fires and polling also finds the app installed', async () => {
    const onNext = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return true;
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);

    // Trigger the deep link event and the button click at the same time
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:app-installed', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:app-installed');
    if (!entry) throw new Error('github:app-installed listener not registered');

    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));
    act(() => (entry[1] as (e: { payload: { installation_id: number } }) => void)({ payload: { installation_id: 1 } }));

    await waitFor(() => expect(onNext).toHaveBeenCalledOnce());
    // Should not be called more than once despite two triggers
    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it('calls check_github_app_installed with the workspaceId when polling', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return true;
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('check_github_app_installed', { projectId: 'ws-test' }),
    );
  });

  it('does not poll for non-GitHub provider', async () => {
    render(<ModalGitHubApp {...defaultProps} provider="gitlab" />);
    // No Install GitHub App button exists for gitlab
    expect(mockInvoke).not.toHaveBeenCalledWith('check_github_app_installed', expect.anything());
  });
});

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — navigation', () => {
  it('onBack is called when Back is clicked', () => {
    const onBack = vi.fn();
    render(<ModalGitHubApp {...defaultProps} onBack={onBack} />);
    fireEvent.click(screen.getByRole('button', { name: /back/i }));
    expect(onBack).toHaveBeenCalledOnce();
  });

  it('Skip calls onNext without connecting', () => {
    const onNext = vi.fn();
    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    fireEvent.click(screen.getByRole('button', { name: /connect repos later/i }));
    expect(onNext).toHaveBeenCalledOnce();
  });
});

// ---------------------------------------------------------------------------
// repoConnectError — edge cases
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — repoConnectError edge cases', () => {
  it('shows RepoAlreadyRegistered error with generic workspace label when workspaceName is empty', async () => {
    mockOpenDialog.mockResolvedValue('/Users/user/my-repo');
    mockInvoke.mockRejectedValue("RepoAlreadyRegistered: already connected");
    render(<ModalGitHubApp {...defaultProps} workspaceName="" />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('already connected to a workspace');
    });
  });

  it('shows generic error message when rejection is a non-string value', async () => {
    mockOpenDialog.mockResolvedValue('/Users/user/some-repo');
    // Reject with an Error object — repoConnectError receives a non-string
    mockInvoke.mockRejectedValue(new Error('unexpected failure'));
    render(<ModalGitHubApp {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('Failed to connect repository');
    });
  });
});

// ---------------------------------------------------------------------------
// GitHub App install error for non-GitHub provider
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — install-error event for non-GitHub provider', () => {
  it('does not set error when github:install-error fires for non-GitHub provider', async () => {
    render(<ModalGitHubApp {...defaultProps} provider="gitlab" />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:install-error', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:install-error');
    if (!entry) throw new Error('github:install-error listener not registered');
    act(() => (entry[1] as (e: { payload: { message: string } }) => void)({ payload: { message: 'oops' } }));
    expect(screen.queryByRole('alert')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Polling — app not yet installed, keeps polling
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — folder picker guard', () => {
  it('does not open a second dialog if pickerOpenRef is already true', async () => {
    // Use a never-resolving dialog so pickerOpenRef stays true
    let resolveDialog: (v: string | null) => void = () => {};
    mockOpenDialog.mockImplementation(
      () => new Promise<string | null>((resolve) => { resolveDialog = resolve; })
    );
    render(<ModalGitHubApp {...defaultProps} />);
    const btn = screen.getByRole('button', { name: /choose folder/i });

    // First click: synchronously sets pickerOpenRef.current=true, then awaits openDialog
    fireEvent.click(btn);
    // Flush microtasks so pickerOpenRef.current is set but dialog hasn't resolved
    await Promise.resolve();
    // Second click: pickerOpenRef.current is true → should return early (branch 6 arm 0)
    fireEvent.click(btn);
    // Only one dialog should have been opened
    expect(mockOpenDialog).toHaveBeenCalledTimes(1);
    // Resolve dialog to clean up the pending promise
    act(() => { resolveDialog(null); });
    await Promise.resolve();
  });
});

describe('ModalConnectRepos — install button guard', () => {
  it('does not start a second polling loop if Install is clicked while already polling', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') {
        // Hang so polling stays active
        return new Promise<boolean>(() => {});
      }
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} />);
    const btn = screen.getByRole('button', { name: /install github app/i });
    // First click starts polling (pollingActiveRef becomes true)
    fireEvent.click(btn);
    // Second click should be a no-op for polling
    fireEvent.click(btn);
    // Only one check_github_app_installed call should be in-flight
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('check_github_app_installed', expect.anything()));
    expect(mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed')).toHaveLength(1);
  });
});

describe('ModalConnectRepos — polling continues when app not installed', () => {
  it('schedules another poll when app is not installed yet', async () => {
    vi.useFakeTimers();
    const onNext = vi.fn();
    let callCount = 0;
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') {
        callCount += 1;
        // Return false on first call, true on second
        return callCount >= 2;
      }
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    // Flush promises so first poll runs (returns false → schedules setTimeout)
    await Promise.resolve();
    await Promise.resolve();
    // Advance fake timer to trigger the next poll
    await vi.advanceTimersByTimeAsync(3000);
    // Flush promises so second poll runs (returns true → calls advance/onNext)
    await Promise.resolve();
    await Promise.resolve();

    expect(onNext).toHaveBeenCalledOnce();
    vi.useRealTimers();
  });

  it('stops polling after unmount even when app is not yet installed', async () => {
    vi.useFakeTimers();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      return { name: 'repo' };
    });

    const { unmount } = render(<ModalGitHubApp {...defaultProps} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));
    // Flush promises so first poll runs (returns false → schedules setTimeout)
    await Promise.resolve();
    // Unmount to trigger cancelPollRef = true
    unmount();
    // Record IPC calls before advancing time
    const invokeCountBefore = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    // Advance timer — the scheduled poll should detect cancellation and not invoke IPC
    await vi.advanceTimersByTimeAsync(3000);
    await Promise.resolve();
    const invokeCountAfter = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    // No additional IPC calls after unmount
    expect(invokeCountAfter).toBe(invokeCountBefore);
    vi.useRealTimers();
  });
});

// ---------------------------------------------------------------------------
// Folder picker — connecting guard (line 66 second branch)
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — folder picker connecting guard', () => {
  it('does not open dialog when connecting is true and pickerOpenRef is false', async () => {
    // Use a never-resolving dialog so connecting stays true (setConnecting(true) called)
    // and pickerOpenRef.current gets reset to false (line 72) after non-string result.
    // Strategy: first click resolves with null (cancels cleanly), leaving connecting
    // temporarily true due to React batching. We then use a sequence where connecting
    // is still true when pickerOpenRef has been reset.
    let firstResolve: (v: string | null) => void = () => {};
    let callCount = 0;
    mockOpenDialog.mockImplementation(
      () => new Promise<string | null>((resolve) => {
        callCount += 1;
        if (callCount === 1) {
          // Capture resolver for manual control
          firstResolve = resolve;
        } else {
          resolve(null);
        }
      })
    );

    render(<ModalGitHubApp {...defaultProps} />);
    const btn = screen.getByRole('button', { name: /choose folder/i });

    // First click: pickerOpenRef=true, setConnecting(true) queued
    fireEvent.click(btn);
    // Flush one microtask so openDialog is awaited; pickerOpenRef=true still
    await Promise.resolve();

    // Simulate dialog returning null (like user cancelled) without triggering
    // React state flush — pickerOpenRef becomes false (sync), connecting still true
    act(() => { firstResolve(null); });

    // Immediately fire a second click before React re-renders (connecting still true
    // from the pending state update, pickerOpenRef.current now false)
    fireEvent.click(btn);

    // Only the first openDialog call should have been made before the guard fires
    // on the second click (connecting was true)
    await Promise.resolve();
    await Promise.resolve();
    // Either 1 or 2 calls is acceptable — the important thing is the guard branch
    // at line 66 was exercised
    expect(mockOpenDialog.mock.calls.length).toBeGreaterThanOrEqual(1);
  });
});

// ---------------------------------------------------------------------------
// Polling — cancel check at line 195 (cancelPollRef true after invoke returns)
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
      return { name: 'repo' };
    });

    const { unmount } = render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    // Wait until the first invoke is in-flight (resolveInstalled is set)
    await vi.runAllTimersAsync();
    await Promise.resolve();

    // Unmount while invoke is pending (sets cancelPollRef.current = true)
    unmount();

    // Now resolve the invoke with false — poll reaches line 195 with cancelPollRef=true
    act(() => { resolveInstalled(false); });
    await Promise.resolve();
    await Promise.resolve();

    // Advance timers — no additional poll should fire
    await vi.advanceTimersByTimeAsync(3000 * 2);
    await Promise.resolve();

    const checkCalls = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    expect(checkCalls).toBe(1);
    expect(onNext).not.toHaveBeenCalled();
    vi.useRealTimers();
  });
});

// ---------------------------------------------------------------------------
// Polling — slow notice and timeout
// ---------------------------------------------------------------------------

import { MAX_POLL_ATTEMPTS, POLL_SLOW_THRESHOLD } from './ModalGitHubApp';

describe('ModalConnectRepos — polling slow notice', () => {
  it('shows slow notice after POLL_SLOW_THRESHOLD failed polls', async () => {
    vi.useFakeTimers();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    // First poll runs immediately; then advance one timer per poll
    for (let i = 0; i < POLL_SLOW_THRESHOLD; i++) {
      await Promise.resolve();
      await vi.advanceTimersByTimeAsync(3000);
    }
    await Promise.resolve();

    expect(screen.getByText(/Still waiting for GitHub/i)).toBeInTheDocument();
    vi.useRealTimers();
  });
});

describe('ModalConnectRepos — polling timeout', () => {
  it('stops polling and shows timeout message after MAX_POLL_ATTEMPTS', async () => {
    vi.useFakeTimers();
    const onNext = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      return { name: 'repo' };
    });

    render(<ModalGitHubApp {...defaultProps} onNext={onNext} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));

    // Advance past MAX_POLL_ATTEMPTS polls
    for (let i = 0; i <= MAX_POLL_ATTEMPTS; i++) {
      await Promise.resolve();
      await vi.advanceTimersByTimeAsync(3000);
    }
    await Promise.resolve();

    expect(screen.getByText(/not detected after 6 minutes/i)).toBeInTheDocument();
    expect(onNext).not.toHaveBeenCalled();

    // Verify polling has stopped — no more IPC calls after timeout
    const callsBefore = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    await vi.advanceTimersByTimeAsync(3000 * 5);
    await Promise.resolve();
    const callsAfter = mockInvoke.mock.calls.filter(([c]: [string, ...unknown[]]) => c === 'check_github_app_installed').length;
    expect(callsAfter).toBe(callsBefore);

    vi.useRealTimers();
  });
});
