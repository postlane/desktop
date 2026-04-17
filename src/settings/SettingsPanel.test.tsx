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
    ...overrides,
  };
}

function setupDefaults() {
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'get_repos') return [makeRepo()];
    if (cmd === 'get_scheduler_credential') throw new Error('not found');
    if (cmd === 'list_profiles_for_repo') return [];
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
// Repos tab — posting account (profile_id) selector
// ---------------------------------------------------------------------------

describe('SettingsPanel — Repos tab — posting account', () => {
  it('shows "Posting account" section for each repo', async () => {
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));
    expect(screen.getByText(/posting account/i)).toBeInTheDocument();
  });

  it('shows "No account selected" when profile_id is empty', async () => {
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
    expect(screen.getByText(/no account selected/i)).toBeInTheDocument();
  });

  it('calls list_profiles_for_repo with the repo id', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'list_profiles_for_repo') return [
        { id: 'p1', name: 'My X Account', platforms: ['x', 'bluesky'] },
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

  it('shows profile names in the dropdown', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'list_profiles_for_repo') return [
        { id: 'p1', name: 'My X Account', platforms: ['x', 'bluesky'] },
        { id: 'p2', name: 'My Alt Account', platforms: ['x'] },
      ];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('My X Account'));
    expect(screen.getByText('My Alt Account')).toBeInTheDocument();
  });

  it('selecting a profile calls save_profile_id with repo id and profile id', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'list_profiles_for_repo') return [
        { id: 'p1', name: 'My X Account', platforms: ['x', 'bluesky'] },
      ];
      if (cmd === 'save_profile_id') return null;
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    const select = await screen.findByRole('combobox', { name: /posting account/i });
    fireEvent.change(select, { target: { value: 'p1' } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_profile_id', { repoId: 'r1', profileId: 'p1' }),
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
});
