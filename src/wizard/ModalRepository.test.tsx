// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
const mockWriteText = vi.fn().mockResolvedValue(undefined);
vi.stubGlobal('navigator', { clipboard: { writeText: mockWriteText } });

import { invoke } from '../ipc/invoke';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
const mockInvoke = vi.mocked(invoke);
const mockOpenDialog = vi.mocked(openDialog);

import ModalRepository from './ModalRepository';

const defaultProps = {
  workspaceId: 'ws-1',
  onBack: vi.fn(),
  onComplete: vi.fn(),
  pollIntervalMs: 10000,
};

beforeEach(() => { vi.clearAllMocks(); });

describe('ModalRepository — folder picker — primary', () => {
  it('test_folder_picker_button_is_primary_action', () => {
    mockInvoke.mockResolvedValue([]);
    render(<ModalRepository {...defaultProps} />);
    expect(screen.getByRole('button', { name: /choose folder/i })).toBeDefined();
  });

  it('test_folder_picker_shows_connecting_during_flight', async () => {
    let resolveFn!: (v: unknown) => void;
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'connect_repo_from_desktop') return new Promise(r => { resolveFn = r; });
      return Promise.resolve([]);
    });
    mockOpenDialog.mockResolvedValue('/some/repo');
    render(<ModalRepository {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => expect(screen.getByText(/connecting/i)).toBeDefined());
    resolveFn({ id: 'r1', name: 'my-repo', path: '/some/repo', active: true, added_at: '' });
  });

  it('test_folder_picker_success_shows_repo_name', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'connect_repo_from_desktop') return Promise.resolve({ id: 'r1', name: 'my-repo', path: '/some/repo', active: true, added_at: '' });
      return Promise.resolve([]);
    });
    mockOpenDialog.mockResolvedValue('/some/repo');
    render(<ModalRepository {...defaultProps} pollIntervalMs={10000} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => expect(screen.getByText(/my-repo/i)).toBeDefined(), { timeout: 3000 });
  });

  it('test_folder_picker_failure_shows_inline_error', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'connect_repo_from_desktop') return Promise.reject('Not a git repository');
      return Promise.resolve([]);
    });
    mockOpenDialog.mockResolvedValue('/some/repo');
    render(<ModalRepository {...defaultProps} pollIntervalMs={10000} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    await waitFor(() => expect(screen.getByText(/not a git repository/i)).toBeDefined(), { timeout: 3000 });
  });

  it('test_cancelled_folder_picker_does_nothing', async () => {
    mockInvoke.mockResolvedValue([]);
    mockOpenDialog.mockResolvedValue(null);
    render(<ModalRepository {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /choose folder/i }));
    expect(screen.queryByText(/connecting/i)).toBeNull();
  });
});

describe('ModalRepository — folder picker — CLI disclosure', () => {
  it('test_cli_command_hidden_initially', () => {
    mockInvoke.mockResolvedValue([]);
    render(<ModalRepository {...defaultProps} />);
    expect(screen.queryByText(/npx @postlane\/cli init/)).toBeNull();
  });

  it('test_cli_disclosure_reveals_command', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<ModalRepository {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /set up manually/i }));
    expect(screen.getByText(/npx @postlane\/cli init/)).toBeDefined();
  });

  it('test_copy_in_disclosure_copies_command', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<ModalRepository {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /set up manually/i }));
    await userEvent.click(screen.getByRole('button', { name: /copy/i }));
    expect(mockWriteText).toHaveBeenCalledWith('npx @postlane/cli init');
  });
});

describe('ModalRepository — detecting phase', () => {
  it('test_renders_cli_command_after_disclosure', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<ModalRepository {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /set up manually/i }));
    expect(screen.getByText(/npx @postlane\/cli init/)).toBeDefined();
  });

  it('test_copy_button_copies_command', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<ModalRepository {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /set up manually/i }));
    await userEvent.click(screen.getByRole('button', { name: /copy/i }));
    expect(mockWriteText).toHaveBeenCalledWith('npx @postlane/cli init');
  });

  it('test_poll_clears_on_unmount', () => {
    mockInvoke.mockResolvedValue([]);
    const clearSpy = vi.spyOn(globalThis, 'clearInterval');
    const { unmount } = render(<ModalRepository {...defaultProps} pollIntervalMs={10000} />);
    unmount();
    expect(clearSpy).toHaveBeenCalled();
    clearSpy.mockRestore();
  });
});

const makeUnlinkedRepo = (name = 'my-repo', path = '/path/my-repo') => ({
  id: 'r1', name, path, active: true, added_at: '',
  path_exists: true, ready_count: 0, failed_count: 0,
  last_post_at: null, provider: null, project_id: null,
});

const makeLinkedRepo = (name = 'existing', path = '/path/existing') => ({
  id: 'r2', name, path, active: true, added_at: '',
  path_exists: true, ready_count: 0, failed_count: 0,
  last_post_at: null, provider: null, project_id: 'some-project-id',
});

// Repo the CLI already linked to this workspace (project_id stamped before register)
const makeWorkspaceLinkedRepo = (name = 'stamped-repo', path = '/path/stamped-repo') => ({
  id: 'r3', name, path, active: true, added_at: '',
  path_exists: true, ready_count: 0, failed_count: 0,
  last_post_at: null, provider: null, project_id: 'ws-1', // matches defaultProps.workspaceId
});

describe('ModalRepository — CLI detection — register params', () => {
  it('test_polling_calls_register_repo_with_project_correct_params', async () => {
    const repo = makeUnlinkedRepo('my-repo', '/path/my-repo');
    mockInvoke
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([repo])
      .mockResolvedValue(undefined);
    render(<ModalRepository {...defaultProps} pollIntervalMs={30} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('register_repo_with_project', {
        projectId: 'ws-1',
        repoPath: '/path/my-repo',
        description: 'my-repo',
      });
    }, { timeout: 3000 });
  });

  it('test_polling_shows_unlinked_repo_name_in_done_view', async () => {
    const repo = makeUnlinkedRepo('my-repo', '/path/my-repo');
    mockInvoke
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([repo])
      .mockResolvedValue(undefined);
    render(<ModalRepository {...defaultProps} pollIntervalMs={30} />);
    await waitFor(() => { expect(screen.getByText(/my-repo/i)).toBeDefined(); }, { timeout: 3000 });
  });

  it('test_polling_skips_repos_with_project_id_set', async () => {
    const repo = makeLinkedRepo();
    mockInvoke.mockResolvedValue([repo]);
    render(<ModalRepository {...defaultProps} pollIntervalMs={30} />);
    await new Promise(r => setTimeout(r, 120));
    expect(mockInvoke).not.toHaveBeenCalledWith('register_repo_with_project', expect.anything());
  });

  it('test_polling_advances_when_repo_already_linked_to_this_workspace', async () => {
    const repo = makeWorkspaceLinkedRepo();
    mockInvoke.mockResolvedValueOnce([]).mockResolvedValue([repo]);
    render(<ModalRepository {...defaultProps} pollIntervalMs={30} />);
    await waitFor(() => { expect(screen.getByText(/stamped-repo/i)).toBeDefined(); }, { timeout: 3000 });
  });

  it('test_polling_does_not_call_register_when_repo_already_linked', async () => {
    const repo = makeWorkspaceLinkedRepo();
    mockInvoke.mockResolvedValueOnce([]).mockResolvedValue([repo]);
    render(<ModalRepository {...defaultProps} pollIntervalMs={30} />);
    await waitFor(() => screen.getByRole('button', { name: /open dashboard/i }), { timeout: 3000 });
    expect(mockInvoke).not.toHaveBeenCalledWith('register_repo_with_project', expect.anything());
  });
});

describe('ModalRepository — done phase', () => {
  it('test_detects_repo_and_transitions_to_done', async () => {
    const repo = makeUnlinkedRepo();
    mockInvoke.mockResolvedValueOnce([]).mockResolvedValueOnce([repo]).mockResolvedValue(undefined);
    render(<ModalRepository {...defaultProps} pollIntervalMs={30} />);
    await waitFor(() => { expect(screen.getByText(/my-repo/i)).toBeDefined(); }, { timeout: 3000 });
  });

  it('test_commit_notice_shown_after_registration', async () => {
    const repo = makeUnlinkedRepo();
    mockInvoke.mockResolvedValueOnce([]).mockResolvedValueOnce([repo]).mockResolvedValue(undefined);
    render(<ModalRepository {...defaultProps} pollIntervalMs={30} />);
    await waitFor(() => { expect(screen.getByText(/commit/i)).toBeDefined(); }, { timeout: 3000 });
  });

  it('test_open_dashboard_calls_set_wizard_completed', async () => {
    const onComplete = vi.fn();
    const repo = makeUnlinkedRepo();
    mockInvoke.mockResolvedValueOnce([]).mockResolvedValueOnce([repo]).mockResolvedValue(undefined);
    render(<ModalRepository {...defaultProps} onComplete={onComplete} pollIntervalMs={30} />);
    await waitFor(() => screen.getByRole('button', { name: /open dashboard/i }), { timeout: 3000 });
    await userEvent.click(screen.getByRole('button', { name: /open dashboard/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_wizard_completed');
      expect(onComplete).toHaveBeenCalledOnce();
    });
  });

  it('test_add_another_repo_restarts_detection', async () => {
    const repo = makeUnlinkedRepo();
    mockInvoke.mockResolvedValueOnce([]).mockResolvedValueOnce([repo]).mockResolvedValue(undefined);
    render(<ModalRepository {...defaultProps} pollIntervalMs={30} />);
    await waitFor(() => screen.getByRole('button', { name: /add another repo/i }), { timeout: 3000 });
    await userEvent.click(screen.getByRole('button', { name: /add another repo/i }));
    expect(screen.getByRole('button', { name: /choose folder/i })).toBeDefined();
  });
});
