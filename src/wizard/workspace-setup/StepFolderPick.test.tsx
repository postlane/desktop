// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MantineProvider } from '@mantine/core';
import '@testing-library/jest-dom';

vi.mock('../../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '../../ipc/invoke';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import StepFolderPick from './StepFolderPick';

const mockInvoke = vi.mocked(invoke);
const mockOpen = vi.mocked(openDialog);

function renderStep(onNext: (workspacePath: string, childRepos: unknown[]) => void) {
  return render(
    <MantineProvider>
      <StepFolderPick onNext={onNext} />
    </MantineProvider>,
  );
}

beforeEach(() => {
  mockInvoke.mockReset();
  mockOpen.mockReset();
});

describe('StepFolderPick', () => {
  it('calls discover_child_repos immediately once a folder is picked', async () => {
    mockOpen.mockResolvedValue('/Users/jordan/code/myorg');
    mockInvoke.mockResolvedValue([
      { name: 'frontend', path: '/Users/jordan/code/myorg/frontend', posts_dir: 'frontend' },
    ]);
    const onNext = vi.fn();
    renderStep(onNext);

    fireEvent.click(screen.getByRole('button', { name: /choose folder/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('discover_child_repos', { path: '/Users/jordan/code/myorg' });
    });
  });

  it('shows an inline error and does not advance when zero repos are found', async () => {
    mockOpen.mockResolvedValue('/Users/jordan/code/empty');
    mockInvoke.mockRejectedValue(
      'No Git repositories found in this folder. Select a folder that contains one or more Git repos.',
    );
    const onNext = vi.fn();
    renderStep(onNext);

    fireEvent.click(screen.getByRole('button', { name: /choose folder/i }));

    await waitFor(() => {
      expect(screen.getByText(/No Git repositories found in this folder/)).toBeInTheDocument();
    });
    expect(onNext).not.toHaveBeenCalled();
  });

  it('does not advance and does not call discover_child_repos when the selection is cleared', async () => {
    mockOpen.mockResolvedValue(null);
    const onNext = vi.fn();
    renderStep(onNext);

    fireEvent.click(screen.getByRole('button', { name: /choose folder/i }));

    await waitFor(() => expect(mockOpen).toHaveBeenCalled());
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(onNext).not.toHaveBeenCalled();
  });

  it('shows discovered repos with their posts_dir and advances on success', async () => {
    mockOpen.mockResolvedValue('/Users/jordan/code/myorg');
    mockInvoke.mockResolvedValue([
      { name: 'frontend', path: '/Users/jordan/code/myorg/frontend', posts_dir: 'frontend' },
      { name: 'frontend', path: '/Users/jordan/other/frontend', posts_dir: 'frontend-2' },
    ]);
    const onNext = vi.fn();
    renderStep(onNext);

    fireEvent.click(screen.getByRole('button', { name: /choose folder/i }));

    await waitFor(() => {
      expect(screen.getAllByText('frontend')).toHaveLength(3); // 2x name column + 1x posts_dir column (first row's name === posts_dir)
      expect(screen.getByText('frontend-2')).toBeInTheDocument();
    });

    expect(onNext).toHaveBeenCalledWith('/Users/jordan/code/myorg', [
      { name: 'frontend', path: '/Users/jordan/code/myorg/frontend', posts_dir: 'frontend' },
      { name: 'frontend', path: '/Users/jordan/other/frontend', posts_dir: 'frontend-2' },
    ]);
  });
});
