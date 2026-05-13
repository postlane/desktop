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
  it('renders step 5 of 5 in WizardShell', () => {
    render(<ModalGitHubApp {...defaultProps} />);
    expect(screen.getByText(/5\s*\/\s*5/)).toBeDefined();
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

  it('shows clean error for RepoAlreadyRegistered', async () => {
    mockOpenDialog.mockResolvedValue('/Users/user/my-repo');
    mockInvoke.mockRejectedValue("RepoAlreadyRegistered: '/Users/user/my-repo' is already registered");
    render(<ModalGitHubApp {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('already connected');
      expect(alert.textContent).not.toContain('RepoAlreadyRegistered:');
    });
  });

  it('does nothing when the folder dialog is cancelled', async () => {
    mockOpenDialog.mockResolvedValue(null);
    render(<ModalGitHubApp {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    expect(mockInvoke).not.toHaveBeenCalled();
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
    fireEvent.click(screen.getByRole('button', { name: /skip/i }));
    expect(onNext).toHaveBeenCalledOnce();
  });
});
