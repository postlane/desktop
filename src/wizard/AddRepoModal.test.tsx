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

describe('AddRepoModal', () => {
  it('renders the modal with Browse button', () => {
    render(<AddRepoModal onClose={vi.fn()} projectId="" />);
    expect(screen.getByRole('button', { name: /browse for the folder/i })).toBeInTheDocument();
  });

  it('calls onClose when Cancel is clicked', () => {
    const onClose = vi.fn();
    render(<AddRepoModal onClose={onClose} projectId="" />);
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('calls connect_repo_from_desktop with repoPath and projectId', async () => {
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockResolvedValue({ id: 'r1', name: 'my-repo', path: '/Users/test/my-repo', active: true, added_at: '2026-01-01T00:00:00Z' });

    render(<AddRepoModal onClose={vi.fn()} projectId="proj-abc" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('connect_repo_from_desktop', {
        repoPath: '/Users/test/my-repo',
        projectId: 'proj-abc',
      })
    );
  });

  it('does not call add_repo or write_project_id_to_config', async () => {
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockResolvedValue({ id: 'r1', name: 'my-repo', path: '/Users/test/my-repo', active: true, added_at: '2026-01-01T00:00:00Z' });

    render(<AddRepoModal onClose={vi.fn()} projectId="proj-abc" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(mockInvoke).not.toHaveBeenCalledWith('add_repo', expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith('write_project_id_to_config', expect.anything());
  });

  it('calls onClose on success', async () => {
    const onClose = vi.fn();
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockResolvedValue({ id: 'r1', name: 'my-repo', path: '/Users/test/my-repo', active: true, added_at: '2026-01-01T00:00:00Z' });

    render(<AddRepoModal onClose={onClose} projectId="" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));

    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it('shows backend error message and stays open when connect_repo_from_desktop rejects', async () => {
    const onClose = vi.fn();
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockRejectedValue('NotAGitRepo: path is not a git repository');

    render(<AddRepoModal onClose={onClose} projectId="" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));

    expect(await screen.findByText(/not a git repository/i)).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});
