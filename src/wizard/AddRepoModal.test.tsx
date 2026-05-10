// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import AddRepoModal from './AddRepoModal';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
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

  it('calls add_repo with the selected path on browse', async () => {
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockResolvedValue({ id: 'r1', name: 'my-repo', path: '/Users/test/my-repo', active: true, added_at: '2026-01-01T00:00:00Z' });

    render(<AddRepoModal onClose={vi.fn()} projectId="" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('add_repo', { path: '/Users/test/my-repo' }));
  });

  it('passes projectId to write_project_id_to_config when projectId is non-empty', async () => {
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockResolvedValue({ id: 'r1', name: 'my-repo', path: '/Users/test/my-repo', active: true, added_at: '2026-01-01T00:00:00Z' });

    render(<AddRepoModal onClose={vi.fn()} projectId="proj-abc" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('write_project_id_to_config', {
        repoPath: '/Users/test/my-repo',
        projectId: 'proj-abc',
      })
    );
  });

  it('does not call write_project_id_to_config when projectId is empty', async () => {
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockResolvedValue({ id: 'r1', name: 'my-repo', path: '/Users/test/my-repo', active: true, added_at: '2026-01-01T00:00:00Z' });

    render(<AddRepoModal onClose={vi.fn()} projectId="" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('add_repo', { path: '/Users/test/my-repo' }));
    expect(mockInvoke).not.toHaveBeenCalledWith('write_project_id_to_config', expect.anything());
  });

  it('shows error and stays open when add_repo rejects', async () => {
    const onClose = vi.fn();
    mockOpen.mockResolvedValue('/Users/test/my-repo');
    mockInvoke.mockRejectedValue(new Error('config.json not found'));

    render(<AddRepoModal onClose={onClose} projectId="" />);
    fireEvent.click(screen.getByRole('button', { name: /browse for the folder/i }));

    expect(await screen.findByText(/run.*postlane init/i)).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});
