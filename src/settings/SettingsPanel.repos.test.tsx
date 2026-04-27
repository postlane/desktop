// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import SettingsPanel from './SettingsPanel';
import type { RepoWithStatus } from '../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
const mockInvoke = vi.mocked(invoke);
const mockDialog = vi.mocked(openDialog);

beforeEach(() => vi.clearAllMocks());

function makeRepo(overrides: Partial<RepoWithStatus> = {}): RepoWithStatus {
  return {
    id: 'r1',
    name: 'my-app',
    path: '/path/to/repo',
    active: true,
    added_at: '2026-01-01T00:00:00Z',
    path_exists: true,
    ready_count: 0,
    failed_count: 0,
    last_post_at: null,
    provider: null,
    ...overrides,
  };
}

describe('SettingsPanel — Repos add and remove — path and add', () => {
  it('renders a not-found repo with Update path and Remove buttons', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo({ path_exists: false })];
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /update path/i })).toBeInTheDocument(),
    );
    expect(screen.getByRole('button', { name: /remove/i })).toBeInTheDocument();
  });

  it('clicking Update path opens picker and calls update_repo_path', async () => {
    mockDialog.mockResolvedValue('/new/path');
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo({ path_exists: false })];
      if (cmd === 'update_repo_path') return null;
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /update path/i }));
    fireEvent.click(screen.getByRole('button', { name: /update path/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_repo_path', expect.objectContaining({ newPath: '/new/path' })),
    );
  });

  it('Add repo calls add_repo when dialog returns a path', async () => {
    const onRepoChange = vi.fn();
    mockDialog.mockResolvedValue('/new/repo');
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'add_repo') return null;
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} onRepoChange={onRepoChange} />);
    await waitFor(() => screen.getByRole('button', { name: /add repo/i }));
    fireEvent.click(screen.getByRole('button', { name: /add repo/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('add_repo', { path: '/new/repo' }),
    );
    await waitFor(() => expect(onRepoChange).toHaveBeenCalledOnce());
  });
});

describe('SettingsPanel — Repos add and remove — confirm dialog', () => {
  it('Remove in Repos confirm dialog calls remove_repo', async () => {
    const onRepoChange = vi.fn();
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'remove_repo') return null;
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} onRepoChange={onRepoChange} />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() => screen.getByRole('dialog'));
    const dialogRemoveBtn = screen.getAllByRole('button', { name: /remove/i }).find(
      (b) => b.closest('[role="dialog"]'),
    );
    expect(dialogRemoveBtn).toBeTruthy();
    if (dialogRemoveBtn) fireEvent.click(dialogRemoveBtn);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('remove_repo', { id: 'r1' }),
    );
    await waitFor(() => expect(onRepoChange).toHaveBeenCalledOnce());
  });

  it('Cancel in Repos Remove dialog closes without deleting', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() => screen.getByRole('dialog'));
    fireEvent.click(screen.getByRole('button', { name: /^cancel$/i }));
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    );
    expect(mockInvoke).not.toHaveBeenCalledWith('remove_repo', expect.anything());
  });

  it('get_repos failure on mount does not crash', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') throw new Error('DB error');
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/no repos registered/i)).toBeInTheDocument(),
    );
  });
});

describe('SettingsPanel — Repos add and remove — error handling', () => {
  it('update_repo_path failure does not crash', async () => {
    mockDialog.mockResolvedValue('/new/path');
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo({ path_exists: false })];
      if (cmd === 'update_repo_path') throw new Error('permission denied');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /update path/i }));
    fireEvent.click(screen.getByRole('button', { name: /update path/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_repo_path', expect.anything()),
    );
  });

  it('Escape key closes the Repos Remove dialog', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() => screen.getByRole('dialog'));
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    );
  });

  it('Remove button on a missing-path repo opens the confirm dialog', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo({ path_exists: false })];
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() => expect(screen.getByRole('dialog')).toBeInTheDocument());
  });
});

describe('SettingsPanel — ReposTab action errors — part 1', () => {
  it('shows error message when toggle active fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'set_repo_active') throw new Error('Watcher restart failed');
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_account_ids') return {};
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));
    fireEvent.click(screen.getByRole('button', { name: /deactivate/i }));
    await waitFor(() =>
      expect(screen.getByText('Watcher restart failed')).toBeInTheDocument()
    );
  });

  it('shows error message when remove repo fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'remove_repo') throw new Error('Permission denied');
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_account_ids') return {};
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));
    fireEvent.click(screen.getByRole('button', { name: /^remove$/i }));
    await waitFor(() => screen.getByRole('dialog'));
    const dialogRemoveBtn = screen.getAllByRole('button', { name: /remove/i }).find(
      (b) => b.closest('[role="dialog"]'),
    );
    if (dialogRemoveBtn) fireEvent.click(dialogRemoveBtn);
    await waitFor(() =>
      expect(screen.getByText('Permission denied')).toBeInTheDocument()
    );
  });
});

describe('SettingsPanel — ReposTab action errors — part 2', () => {
  it('shows error message when add_repo fails', async () => {
    mockDialog.mockResolvedValue('/new/repo');
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'add_repo') throw new Error('Path is not a git repo');
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_account_ids') return {};
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /add repo/i }));
    fireEvent.click(screen.getByRole('button', { name: /add repo/i }));
    await waitFor(() =>
      expect(screen.getByText('Path is not a git repo')).toBeInTheDocument()
    );
  });

  it('clears previous error when a new action starts', async () => {
    let callCount = 0;
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'set_repo_active') {
        callCount++;
        if (callCount === 1) throw new Error('First failure');
        return null;
      }
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_account_ids') return {};
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));
    fireEvent.click(screen.getByRole('button', { name: /deactivate/i }));
    await waitFor(() => expect(screen.getByText('First failure')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /deactivate|activate/i }));
    await waitFor(() =>
      expect(screen.queryByText('First failure')).not.toBeInTheDocument()
    );
  });
});

describe('SettingsPanel — ReposTab toggle in-flight guard', () => {
  it('second toggle click is ignored while first is in flight', async () => {
    let resolve!: () => void;
    const blocker = new Promise<void>((r) => { resolve = r; });
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'set_repo_active') { await blocker; return null; }
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_account_ids') return {};
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));
    const btn = screen.getByRole('button', { name: /deactivate/i });
    fireEvent.click(btn);
    fireEvent.click(btn);
    resolve();
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_repo_active', expect.anything()),
    );
    const toggleCalls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'set_repo_active');
    expect(toggleCalls).toHaveLength(1);
  });

  it('toggle button is disabled while in flight', async () => {
    let resolve!: () => void;
    const blocker = new Promise<void>((r) => { resolve = r; });
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'set_repo_active') { await blocker; return null; }
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_account_ids') return {};
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));
    const btn = screen.getByRole('button', { name: /deactivate/i });
    fireEvent.click(btn);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /deactivate/i })).toBeDisabled(),
    );
    resolve();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /deactivate|activate/i })).not.toBeDisabled(),
    );
  });
});
