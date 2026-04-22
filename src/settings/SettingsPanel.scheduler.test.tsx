// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import SettingsPanel from './SettingsPanel';
import type { RepoWithStatus } from '../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

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
    ...overrides,
  };
}

describe('SettingsPanel — Scheduler credential removal — add and cancel', () => {
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
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_account_ids') return {};
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
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
});

describe('SettingsPanel — Scheduler credential removal — cancel and change', () => {
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
});

describe('SettingsPanel — Scheduler credential removal — form cancel and empty save', () => {
  it('Cancel in the key input form hides it without saving', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_account_ids') return {};
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
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
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_account_ids') return {};
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    fireEvent.click(await screen.findByRole('tab', { name: /scheduler/i }));
    await waitFor(() => screen.getAllByRole('button', { name: /add/i }));
    fireEvent.click(screen.getAllByRole('button', { name: /add/i })[0]);
    await waitFor(() => screen.getByPlaceholderText(/api key/i));
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    expect(mockInvoke).not.toHaveBeenCalledWith('save_scheduler_credential', expect.anything());
  });
});

describe('SettingsPanel — Scheduler credential removal — failure paths', () => {
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
    expect(dialogRemoveBtn).toBeTruthy();
    if (dialogRemoveBtn) fireEvent.click(dialogRemoveBtn);
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
  });
});

describe('SettingsPanel — Scheduler credential removal — confirm and delete', () => {
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
    expect(dialogRemoveBtn).toBeTruthy();
    if (dialogRemoveBtn) fireEvent.click(dialogRemoveBtn);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('delete_scheduler_credential', { provider: 'zernio' }),
    );
  });

  it('test_scheduler non-Error exception shows "Test failed" fallback', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo()];
      if (cmd === 'get_scheduler_credential') return '••••abcd';
      if (cmd === 'test_scheduler') throw 'connection refused';
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

describe('SettingsPanel — attribution toggle — state', () => {
  it('renders an attribution toggle in the App tab', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_attribution') return true;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('tab', { name: /app/i }));
    fireEvent.click(screen.getByRole('tab', { name: /app/i }));
    await waitFor(() =>
      expect(screen.getByRole('switch', { name: /post attribution/i })).toBeInTheDocument(),
    );
  });

  it('attribution toggle is on by default (when get_attribution returns true)', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_attribution') return true;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('tab', { name: /app/i }));
    fireEvent.click(screen.getByRole('tab', { name: /app/i }));
    await waitFor(() => screen.getByRole('switch', { name: /post attribution/i }));
    expect(screen.getByRole('switch', { name: /post attribution/i })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  });

  it('toggling attribution off calls set_attribution with false', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_attribution') return true;
      if (cmd === 'set_attribution') return null;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('tab', { name: /app/i }));
    fireEvent.click(screen.getByRole('tab', { name: /app/i }));
    await waitFor(() => screen.getByRole('switch', { name: /post attribution/i }));
    fireEvent.click(screen.getByRole('switch', { name: /post attribution/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_attribution', { enabled: false }),
    );
  });
});

describe('SettingsPanel — attribution toggle — on and error', () => {
  it('toggling attribution back on calls set_attribution with true', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_attribution') return false;
      if (cmd === 'set_attribution') return null;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('tab', { name: /app/i }));
    fireEvent.click(screen.getByRole('tab', { name: /app/i }));
    await waitFor(() => screen.getByRole('switch', { name: /post attribution/i }));
    fireEvent.click(screen.getByRole('switch', { name: /post attribution/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_attribution', { enabled: true }),
    );
  });

  it('does not crash when set_attribution throws', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [];
      if (cmd === 'get_attribution') return true;
      if (cmd === 'set_attribution') throw new Error('keyring unavailable');
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('tab', { name: /app/i }));
    fireEvent.click(screen.getByRole('tab', { name: /app/i }));
    await waitFor(() => screen.getByRole('switch', { name: /post attribution/i }));
    fireEvent.click(screen.getByRole('switch', { name: /post attribution/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_attribution', { enabled: false }),
    );
    expect(screen.getByRole('switch', { name: /post attribution/i })).toBeInTheDocument();
  });
});

describe('SettingsPanel — ReposTab credential version bump', () => {
  it('saving a scheduler credential in the Repos tab bumps the credential version', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_repos') return [makeRepo({ provider: 'zernio' })];
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'save_scheduler_credential') return null;
      if (cmd === 'list_profiles_for_repo') return [];
      if (cmd === 'get_account_ids') return {};
      if (cmd === 'get_app_version') return '0.1.0';
      if (cmd === 'get_autostart_enabled') return false;
      return null;
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    await waitFor(() => screen.getByText('my-app'));
    fireEvent.click(await screen.findByRole('button', { name: /override for this repo/i }));
    const input = await screen.findByPlaceholderText(/paste api key/i);
    fireEvent.change(input, { target: { value: 'sk-zernio-test' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'save_scheduler_credential',
        expect.objectContaining({ apiKey: 'sk-zernio-test' }),
      ),
    );
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('list_profiles_for_repo', expect.anything()),
    );
  });
});
