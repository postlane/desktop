// SPDX-License-Identifier: BUSL-1.1
// Tests for the folder-conflict path in ModalGitHubApp (Bug 21.13.6a).
//
// When connect_repo_from_desktop returns RepoAlreadyRegistered, the component
// must call find_project_for_folder to discover which project owns the folder,
// then invoke onFolderAlreadyConnected so the wizard can redirect to that
// project instead of advancing with the newly-created empty one.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), message: vi.fn().mockResolvedValue(undefined) }));
vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));

import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { invoke } from '../ipc/invoke';
import ModalGitHubApp from './ModalGitHubApp';

const mockOpenDialog = vi.mocked(openDialog);
const mockInvoke = vi.mocked(invoke);

const defaultProps = {
  provider: 'github',
  workspaceId: 'ws-new-empty',
  workspaceName: 'new-org',
  onNext: vi.fn(),
  onBack: vi.fn(),
  setRepoConnected: vi.fn(),
};

beforeEach(() => {
  vi.clearAllMocks();
  mockOpenDialog.mockResolvedValue('/Users/test/my-repo');
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'check_github_app_installed') return false;
    if (cmd === 'list_repos_for_project') return [];
    if (cmd === 'find_project_for_folder') return null;
    if (cmd === 'connect_repo_from_desktop') throw 'RepoAlreadyRegistered: already registered';
    return null;
  });
});

// --- §find_project_for_folder called on conflict ---

describe('ModalGitHubApp — folder conflict (21.13.6a)', () => {
  it('calls find_project_for_folder with the selected path when RepoAlreadyRegistered', async () => {
    render(<ModalGitHubApp {...defaultProps} />);
    fireEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('find_project_for_folder', {
        folderPath: '/Users/test/my-repo',
      })
    );
  });

  it('invokes onFolderAlreadyConnected with existing project id when found', async () => {
    const onFolderAlreadyConnected = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false;
      if (cmd === 'list_repos_for_project') return [];
      if (cmd === 'find_project_for_folder') return 'proj-existing-abc';
      if (cmd === 'connect_repo_from_desktop') throw 'RepoAlreadyRegistered: already registered';
      return null;
    });
    render(<ModalGitHubApp {...defaultProps} onFolderAlreadyConnected={onFolderAlreadyConnected} />);
    fireEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() =>
      expect(onFolderAlreadyConnected).toHaveBeenCalledWith('proj-existing-abc')
    );
  });

  it('does not call onFolderAlreadyConnected when find_project_for_folder returns null', async () => {
    const onFolderAlreadyConnected = vi.fn();
    render(<ModalGitHubApp {...defaultProps} onFolderAlreadyConnected={onFolderAlreadyConnected} />);
    fireEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('find_project_for_folder', expect.anything())
    );
    expect(onFolderAlreadyConnected).not.toHaveBeenCalled();
  });

  it('shows "another workspace" in error message — not the new workspace name', async () => {
    render(<ModalGitHubApp {...defaultProps} workspaceName="new-org" />);
    fireEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('another workspace');
      expect(alert.textContent).not.toContain('new-org');
    });
  });
});
