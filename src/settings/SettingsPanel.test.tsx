// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import SettingsPanel from './SettingsPanel';
import type { RepoWithStatus } from '../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

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

function setupDefaults() {
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'get_repos') return [makeRepo()];
    if (cmd === 'get_scheduler_credential') throw new Error('not found');
    if (cmd === 'list_profiles_for_repo') return [];
    if (cmd === 'get_account_ids') return {};
    if (cmd === 'get_app_version') return '0.1.0';
    if (cmd === 'get_autostart_enabled') return false;
    return null;
  });
}

describe('SettingsPanel — tabs', () => {
  it('renders three tabs: Repos, Scheduler, App', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('tab', { name: /repos/i }));
    expect(screen.getByRole('tab', { name: /scheduler/i })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /app/i })).toBeInTheDocument();
  });

  it('defaults to Repos tab', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('tab', { name: /repos/i, selected: true }));
  });

  it('clicking Scheduler tab shows scheduler content', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('tab', { name: /scheduler/i }));
    fireEvent.click(screen.getByRole('tab', { name: /scheduler/i }));
    await waitFor(() =>
      expect(screen.getByText(/zernio/i)).toBeInTheDocument(),
    );
  });

  it('clicking App tab shows app content', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('tab', { name: /app/i }));
    fireEvent.click(screen.getByRole('tab', { name: /app/i }));
    await waitFor(() =>
      expect(screen.getByText(/launch at login/i)).toBeInTheDocument(),
    );
  });
});

describe('SettingsPanel — Repos tab — display', () => {
  it('shows repo name and path', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));
    expect(screen.getByText('/path/to/repo')).toBeInTheDocument();
  });

  it('shows active status indicator', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));
    expect(screen.getByTitle(/active/i)).toBeInTheDocument();
  });

  it('shows not found repo with (missing) label', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo({ path_exists: false })];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText(/missing/i));
  });

  it('shows [Add repo] button', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /add repo/i })).toBeInTheDocument(),
    );
  });
});

describe('SettingsPanel — Repos tab — actions', () => {
  it('[Add repo] opens folder picker and calls add_repo', async () => {
    setupDefaults();
    mockDialog.mockResolvedValue('/new/repo');
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'add_repo') return makeRepo({ id: 'r2', name: 'new-repo', path: '/new/repo' });
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /add repo/i }));
    fireEvent.click(screen.getByRole('button', { name: /add repo/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('add_repo', expect.anything()),
    );
  });

  it('[Remove] shows confirmation before removing', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() =>
      expect(screen.getByRole('dialog')).toBeInTheDocument(),
    );
  });

  it('[Deactivate] calls set_repo_active with false', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /deactivate/i }));
    fireEvent.click(screen.getByRole('button', { name: /deactivate/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_repo_active', { id: 'r1', active: false }),
    );
  });
});

describe('SettingsPanel — Scheduler tab', () => {
  it('shows all three v1 providers', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('tab', { name: /scheduler/i }));
    fireEvent.click(screen.getByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getByText(/zernio/i));
    expect(screen.getByText(/buffer/i)).toBeInTheDocument();
    expect(screen.getByText(/ayrshare/i)).toBeInTheDocument();
  });

  it('shows "not configured" when no credential', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() =>
      expect(screen.getAllByText(/not configured/i).length).toBeGreaterThan(0),
    );
  });

  it('shows [+ Add] button for unconfigured provider', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() =>
      expect(screen.getAllByRole('button', { name: /add/i }).length).toBeGreaterThan(0),
    );
  });

  it('[Test] calls test_scheduler and shows result', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') return '••••abcd';
      if (cmd === 'test_scheduler') return true;
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /test/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /test/i })[0]);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('test_scheduler', expect.anything()),
    );
  });
});

describe('SettingsPanel — Repos tab — posting accounts display', () => {
  it('shows "Posting accounts" section for each repo', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));
    expect(screen.getByText(/posting accounts/i)).toBeInTheDocument();
  });

  it('shows "No accounts connected" when list returns empty', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));
    expect(screen.getByText(/no accounts connected/i)).toBeInTheDocument();
  });

  it('calls list_profiles_for_repo with the repo id', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'list_profiles_for_repo') return [
        { id: 'acc-twitter-1', name: '@myhandle', platforms: ['twitter'] },
      ];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('list_profiles_for_repo', { repoId: 'r1' }),
    );
  });
});

describe('SettingsPanel — Repos tab — posting accounts select', () => {
  it('shows account names in per-platform dropdowns', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'list_profiles_for_repo') return [
        { id: 'acc-twitter-1', name: '@myhandle', platforms: ['twitter'] },
        { id: 'acc-bsky-1', name: '@myhandle.bsky.social', platforms: ['bluesky'] },
      ];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('@myhandle'));
    expect(screen.getByText('@myhandle.bsky.social')).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: /X account/i })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: /Bluesky account/i })).toBeInTheDocument();
  });

  it('selecting a platform account calls save_account_id', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'list_profiles_for_repo') return [
        { id: 'acc-twitter-1', name: '@myhandle', platforms: ['twitter'] },
      ];
      if (cmd === 'save_account_id') return null;
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    const select = await screen.findByRole('combobox', { name: /X account/i });
    fireEvent.change(select, { target: { value: 'acc-twitter-1' } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_account_id', {
        repoId: 'r1',
        platform: 'twitter',
        accountId: 'acc-twitter-1',
      }),
    );
  });

  it('shows error message if list_profiles_for_repo fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'list_profiles_for_repo') throw new Error('No API key configured');
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/no api key configured/i)).toBeInTheDocument(),
    );
  });
});
