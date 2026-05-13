// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import AddRepoModal from './AddRepoModal';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '../ipc/invoke';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

const mockInvoke = vi.mocked(invoke);
const mockOpen = vi.mocked(openDialog);

beforeEach(() => vi.clearAllMocks());

const successRepo = { id: 'r1', name: 'my-repo', path: '/Users/test/my-repo', active: true, added_at: '2026-01-01T00:00:00Z' };

describe('AddRepoModal — structure and navigation', () => {
  it('renders the modal with Browse button', () => {
    render(<AddRepoModal onClose={vi.fn()} projectId="" projectName="" />);
    expect(screen.getByRole('button', { name: /browse for the folder/i })).toBeInTheDocument();
  });

  it('calls onClose when Cancel is clicked', () => {
    const onClose = vi.fn();
    render(<AddRepoModal onClose={onClose} projectId="" projectName="" />);
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});

describe('AddRepoModal — folder connect', () => {
  it('calls connect_repo_from_desktop with repoPath and projectId', async () => {
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockResolvedValue(successRepo);
    render(<AddRepoModal onClose={vi.fn()} projectId="proj-abc" projectName="postlane" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('connect_repo_from_desktop', {
        repoPath: '/Users/test/my-repo',
        projectId: 'proj-abc',
      })
    );
  });

  it('does not call legacy add_repo or write_project_id_to_config commands', async () => {
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockResolvedValue(successRepo);
    render(<AddRepoModal onClose={vi.fn()} projectId="proj-abc" projectName="postlane" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(mockInvoke).not.toHaveBeenCalledWith('add_repo', expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith('write_project_id_to_config', expect.anything());
  });

  it('shows repo name and Done button on success without auto-closing', async () => {
    const onClose = vi.fn();
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockResolvedValue(successRepo);
    render(<AddRepoModal onClose={onClose} projectId="" projectName="" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));
    await waitFor(() => expect(screen.getByText('my-repo')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: /done/i })).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('calls onClose when Done is clicked after success', async () => {
    const onClose = vi.fn();
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockResolvedValue(successRepo);
    render(<AddRepoModal onClose={onClose} projectId="" projectName="" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));
    await waitFor(() => screen.getByRole('button', { name: /done/i }));
    fireEvent.click(screen.getByRole('button', { name: /done/i }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});

describe('AddRepoModal — error messages', () => {
  it('shows clean error for NotAGitRepo', async () => {
    mockOpen.mockResolvedValue('/Users/test/not-a-repo');
    mockInvoke.mockRejectedValue("NotAGitRepo: '/Users/test/not-a-repo' is not a git repository");
    render(<AddRepoModal onClose={vi.fn()} projectId="" projectName="" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('Not a Git repository');
      expect(alert.textContent).not.toContain('NotAGitRepo:');
    });
  });

  it('shows clean error for RepoAlreadyRegistered including workspace name', async () => {
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockRejectedValue("RepoAlreadyRegistered: '/Users/test/my-repo' is already registered");
    render(<AddRepoModal onClose={vi.fn()} projectId="" projectName="my-org" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('already connected to the my-org workspace');
      expect(alert.textContent).not.toContain('RepoAlreadyRegistered:');
    });
  });

  it('shows clean error for PathNotAuthorised', async () => {
    mockOpen.mockResolvedValue('/etc/secrets');
    mockInvoke.mockRejectedValue("PathNotAuthorised: '/etc/secrets' is outside the home directory");
    render(<AddRepoModal onClose={vi.fn()} projectId="" projectName="" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('outside your home directory');
      expect(alert.textContent).not.toContain('PathNotAuthorised:');
    });
  });
});
