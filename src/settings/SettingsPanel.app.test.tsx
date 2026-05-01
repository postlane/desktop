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
const mockInvoke = vi.mocked(invoke);

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
    project_id: null,
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

describe('SettingsPanel — App tab — basics', () => {
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
    setupDefaults();
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    const checkbox = await screen.findByRole('checkbox', { name: /launch at login/i });
    fireEvent.click(checkbox);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('plugin:autostart|enable'),
    );
  });
});

describe('SettingsPanel — App tab — autostart and logs', () => {
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
});

describe('SettingsPanel — App tab — updates and timezone', () => {
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
});

describe('SettingsPanel — App tab — error handling', () => {
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
  });
});
