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

// ---------------------------------------------------------------------------
// Container — three tabs
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Repos tab
// ---------------------------------------------------------------------------

describe('SettingsPanel — Repos tab', () => {
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

// ---------------------------------------------------------------------------
// Scheduler tab
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Repos tab — posting accounts (per-platform) selector
// ---------------------------------------------------------------------------

describe('SettingsPanel — Repos tab — posting account', () => {
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
    // One combobox per platform
    expect(screen.getByRole('combobox', { name: /X account/i })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: /Bluesky account/i })).toBeInTheDocument();
  });

  it('selecting a platform account calls save_account_id with platform and accountId', async () => {
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

// ---------------------------------------------------------------------------
// App tab
// ---------------------------------------------------------------------------

describe('SettingsPanel — App tab', () => {
  it('shows launch at login toggle', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    await waitFor(() =>
      expect(screen.getByRole('checkbox', { name: /launch at login/i })).toBeInTheDocument(),
    );
  });

  it('shows version string', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    await waitFor(() =>
      expect(screen.getByText(/postlane 0\.1\.0/i)).toBeInTheDocument(),
    );
  });

  it('shows Open log folder button', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /open log folder/i })).toBeInTheDocument(),
    );
  });

  it('shows Check for updates button', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /check for updates/i })).toBeInTheDocument(),
    );
  });

  it('toggling autostart calls the enable plugin command', async () => {
    setupDefaults(); // autostart=false by default
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    const checkbox = await screen.findByRole('checkbox', { name: /launch at login/i });
    fireEvent.click(checkbox);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('plugin:autostart|enable'),
    );
  });

  it('toggling autostart when enabled calls the disable plugin command', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return true;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    const checkbox = await screen.findByRole('checkbox', { name: /launch at login/i });
    fireEvent.click(checkbox);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('plugin:autostart|disable'),
    );
  });

  it('clicking Open log folder calls the opener plugin', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    await waitFor(() => screen.getByRole('button', { name: /open log folder/i }));
    fireEvent.click(screen.getByRole('button', { name: /open log folder/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('plugin:opener|open_path', expect.anything()),
    );
  });

  it('Check for updates shows "You are up to date." on no update', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      if (cmd === 'plugin:updater|check') return null;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    await waitFor(() => screen.getByRole('button', { name: /check for updates/i }));
    fireEvent.click(screen.getByRole('button', { name: /check for updates/i }));
    await waitFor(() =>
      expect(screen.getByText(/you are up to date/i)).toBeInTheDocument(),
    );
  });

  it('Check for updates shows available version when update exists', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      if (cmd === 'plugin:updater|check') return '0.2.0';
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    await waitFor(() => screen.getByRole('button', { name: /check for updates/i }));
    fireEvent.click(screen.getByRole('button', { name: /check for updates/i }));
    await waitFor(() =>
      expect(screen.getByText(/update available/i)).toBeInTheDocument(),
    );
  });

  it('Check for updates shows error when check fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      if (cmd === 'plugin:updater|check') throw new Error('network');
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    await waitFor(() => screen.getByRole('button', { name: /check for updates/i }));
    fireEvent.click(screen.getByRole('button', { name: /check for updates/i }));
    await waitFor(() =>
      expect(screen.getByText(/could not check for updates/i)).toBeInTheDocument(),
    );
  });

  it('changing timezone calls read_app_state_command and save_app_state_command', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      if (cmd === 'read_app_state_command') return { timezone: 'UTC' };
      if (cmd === 'save_app_state_command') return null;
      return null;
    });
    const onTimezoneChange = vi.fn();
    render(<SettingsPanel onClose={vi.fn()} onTimezoneChange={onTimezoneChange} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    const select = await screen.findByRole('combobox', { name: /display timezone/i });
    fireEvent.change(select, { target: { value: 'America/New_York' } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_app_state_command', expect.anything()),
    );
    await waitFor(() =>
      expect(onTimezoneChange).toHaveBeenCalledWith('America/New_York'),
    );
  });

  it('timezone save failure does not crash', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      if (cmd === 'read_app_state_command') throw new Error('state missing');
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    const select = await screen.findByRole('combobox', { name: /display timezone/i });
    fireEvent.change(select, { target: { value: 'America/New_York' } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('read_app_state_command'),
    );
    // No crash
  });

  it('autostart toggle failure does not crash', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      if (cmd === 'plugin:autostart|enable') throw new Error('autostart failed');
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    const checkbox = await screen.findByRole('checkbox', { name: /launch at login/i });
    fireEvent.click(checkbox);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('plugin:autostart|enable'),
    );
    // No crash
  });

  it('open logs failure does not crash', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      if (cmd === 'plugin:opener|open_path') throw new Error('no opener');
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    await waitFor(() => screen.getByRole('button', { name: /open log folder/i }));
    fireEvent.click(screen.getByRole('button', { name: /open log folder/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('plugin:opener|open_path', expect.anything()),
    );
    // No crash
  });
});

// ---------------------------------------------------------------------------
// Repos tab — add / remove
// ---------------------------------------------------------------------------

describe('SettingsPanel — Repos tab — add and remove', () => {
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
    fireEvent.click(dialogRemoveBtn!);
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
    // No crash
  });

  it('Escape key closes the Repos Remove dialog via onClose', async () => {
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

// ---------------------------------------------------------------------------
// ReposTab — actionError display
// ---------------------------------------------------------------------------

describe('SettingsPanel — ReposTab action errors', () => {
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
    // Open confirm dialog
    fireEvent.click(screen.getByRole('button', { name: /^remove$/i }));
    // Confirm removal
    await waitFor(() => screen.getByRole('dialog'));
    const dialogRemoveBtn = screen.getAllByRole('button', { name: /remove/i }).find(
      (b) => b.closest('[role="dialog"]'),
    );
    if (dialogRemoveBtn) fireEvent.click(dialogRemoveBtn);
    await waitFor(() =>
      expect(screen.getByText('Permission denied')).toBeInTheDocument()
    );
  });

  it('clears previous error when a new action starts', async () => {
    let callCount = 0;
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'set_repo_active') {
        callCount++;
        if (callCount === 1) throw new Error('First failure');
        return null; // second call succeeds
      }
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_account_ids') return {};
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));

    // First toggle fails — error appears
    fireEvent.click(screen.getByRole('button', { name: /deactivate/i }));
    await waitFor(() => expect(screen.getByText('First failure')).toBeInTheDocument());

    // Second toggle succeeds — error clears
    fireEvent.click(screen.getByRole('button', { name: /deactivate|activate/i }));
    await waitFor(() =>
      expect(screen.queryByText('First failure')).not.toBeInTheDocument()
    );
  });
});

// ---------------------------------------------------------------------------
// Scheduler tab — type-to-confirm removal
// ---------------------------------------------------------------------------

describe('SettingsPanel — Scheduler tab — credential removal', () => {
  it('Remove button is disabled until provider name is typed', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') return '••••abcd';
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /remove/i })[0]);
    await waitFor(() => screen.getByRole('dialog'));
    const removeBtn = screen.getAllByRole('button', { name: /remove/i }).find(
      (b) => b.closest('[role="dialog"]'),
    );
    expect(removeBtn).toBeDisabled();
  });

  it('clicking + Add shows the API key input', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /add/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /add/i })[0]);
    await waitFor(() =>
      expect(screen.getByPlaceholderText(/api key/i)).toBeInTheDocument(),
    );
  });

  it('saving a new API key calls save_scheduler_credential', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'save_scheduler_credential') return null;
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /add/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /add/i })[0]);
    const input = await screen.findByPlaceholderText(/api key/i);
    fireEvent.change(input, { target: { value: 'sk-test-1234' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_scheduler_credential', expect.objectContaining({ apiKey: 'sk-test-1234' })),
    );
  });

  it('clicking Cancel in the remove dialog closes it without deleting', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') return '••••abcd';
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /remove/i })[0]);
    await waitFor(() => screen.getByRole('dialog'));
    fireEvent.click(screen.getByRole('button', { name: /^cancel$/i }));
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    );
    expect(mockInvoke).not.toHaveBeenCalledWith('delete_scheduler_credential', expect.anything());
  });

  it('clicking Change when credential exists shows the key input form', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') return '••••abcd';
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    const changeBtn = await screen.findAllByRole('button', { name: /^change$/i });
    fireEvent.click(changeBtn[0]);
    await waitFor(() =>
      expect(screen.getByPlaceholderText(/api key/i)).toBeInTheDocument(),
    );
  });

  it('Cancel in the key input form hides it without saving', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /add/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /add/i })[0]);
    await waitFor(() => screen.getByPlaceholderText(/api key/i));
    fireEvent.click(screen.getByRole('button', { name: /^cancel$/i }));
    await waitFor(() =>
      expect(screen.queryByPlaceholderText(/api key/i)).not.toBeInTheDocument(),
    );
  });

  it('clicking Save with empty key does nothing', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /add/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /add/i })[0]);
    await waitFor(() => screen.getByPlaceholderText(/api key/i));
    // don't type anything — click Save with empty input
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    expect(mockInvoke).not.toHaveBeenCalledWith('save_scheduler_credential', expect.anything());
  });

  it('delete_scheduler_credential failure does not crash', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') return '••••abcd';
      if (cmd === 'delete_scheduler_credential') throw new Error('Keychain locked');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /remove/i })[0]);
    await waitFor(() => screen.getByRole('dialog'));
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'zernio' } });
    const dialogRemoveBtn = screen.getAllByRole('button', { name: /remove/i }).find(
      (b) => !b.hasAttribute('disabled') && b.closest('[role="dialog"]'),
    );
    fireEvent.click(dialogRemoveBtn!);
    // No crash — dialog should close or stay open; test just verifies no throw
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('delete_scheduler_credential', { provider: 'zernio' }),
    );
  });

  it('save_scheduler_credential failure does not crash', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'save_scheduler_credential') throw new Error('Keychain error');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /add/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /add/i })[0]);
    const input = await screen.findByPlaceholderText(/api key/i);
    fireEvent.change(input, { target: { value: 'sk-bad-key' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_scheduler_credential', expect.anything()),
    );
    // No crash
  });

  it('typing the provider name enables the Remove button and confirms deletion', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') return '••••abcd';
      if (cmd === 'test_scheduler') return true;
      if (cmd === 'delete_scheduler_credential') return null;
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /remove/i })[0]);
    await waitFor(() => screen.getByRole('dialog'));
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'zernio' } });
    const dialogRemoveBtn = screen.getAllByRole('button', { name: /remove/i }).find(
      (b) => !b.hasAttribute('disabled') && b.closest('[role="dialog"]'),
    );
    expect(dialogRemoveBtn).not.toBeDisabled();
    fireEvent.click(dialogRemoveBtn!);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('delete_scheduler_credential', { provider: 'zernio' }),
    );
  });

  it('test_scheduler non-Error exception shows "Test failed" fallback', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') return '••••abcd';
      if (cmd === 'test_scheduler') throw 'connection refused'; // non-Error throw
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /test/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /test/i })[0]);
    await waitFor(() =>
      expect(screen.getByText(/test failed/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// ReposTab — toggle in-flight guard
// ---------------------------------------------------------------------------

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
