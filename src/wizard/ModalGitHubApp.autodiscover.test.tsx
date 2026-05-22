// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, waitFor, act } from '@testing-library/react';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), message: vi.fn().mockResolvedValue(undefined) }));
vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));

import { listen } from '@tauri-apps/api/event';
import { message } from '@tauri-apps/plugin-dialog';
import { invoke } from '../ipc/invoke';
import ModalGitHubApp from './ModalGitHubApp';

const mockListen = vi.mocked(listen);
const mockMessage = vi.mocked(message);
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
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'check_github_app_installed') return false;
    if (cmd === 'list_repos_for_project') return [];
    return { name: 'my-repo' };
  });
});

// ---------------------------------------------------------------------------
// Auto-discover repos on GitHub App install (21.10.12–21.10.13)
// ---------------------------------------------------------------------------

describe('ModalConnectRepos — auto-discover repos on app install (21.10.12–21.10.13)', () => {
  it('calls discover_repos with projectId after github:app-installed event fires (21.10.12)', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      if (cmd === 'list_repos_for_project') return [];
      if (cmd === 'discover_repos') return { added: [], already_registered: [], not_found_on_disk: [], failed_to_register: [] };
      return { name: 'my-repo' };
    });
    render(<ModalGitHubApp {...defaultProps} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:app-installed', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:app-installed');
    if (!entry) throw new Error('github:app-installed listener not registered');
    act(() => (entry[1] as (e: { payload: { installation_id: number } }) => void)({ payload: { installation_id: 1 } }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('discover_repos', { projectId: 'ws-test' }));
  });

  it('shows message toast when discover_repos returns added repos (21.10.13)', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      if (cmd === 'list_repos_for_project') return [];
      if (cmd === 'discover_repos') return { added: ['my-repo'], already_registered: [], not_found_on_disk: [], failed_to_register: [] };
      return { name: 'my-repo' };
    });
    render(<ModalGitHubApp {...defaultProps} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:app-installed', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:app-installed');
    if (!entry) throw new Error('github:app-installed listener not registered');
    act(() => (entry[1] as (e: { payload: { installation_id: number } }) => void)({ payload: { installation_id: 1 } }));
    await waitFor(() => expect(mockMessage).toHaveBeenCalledWith('Found and registered 1 repo(s) on your machine.'));
  });

  it('shows failed_to_register toast when discover_repos returns failures (21.10.13)', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      if (cmd === 'list_repos_for_project') return [];
      if (cmd === 'discover_repos') return { added: [], already_registered: [], not_found_on_disk: [], failed_to_register: [['my-repo', 'permission denied']] };
      return { name: 'my-repo' };
    });
    render(<ModalGitHubApp {...defaultProps} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:app-installed', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:app-installed');
    if (!entry) throw new Error('github:app-installed listener not registered');
    act(() => (entry[1] as (e: { payload: { installation_id: number } }) => void)({ payload: { installation_id: 1 } }));
    await waitFor(() => expect(mockMessage).toHaveBeenCalledWith('1 repo(s) could not be registered — open Repositories to see details'));
  });
});
