// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { TimezoneContext } from '../TimezoneContext';
import SettingsPanel from './SettingsPanel';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

function setupMocks(defaultPostTime: { hour: number; minute: number } | null = null) {
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'get_repos') return [];
    if (cmd === 'get_scheduler_credential') throw new Error('not found');
    if (cmd === 'get_app_version') return '0.1.0';
    if (cmd === 'get_autostart_enabled') return false;
    if (cmd === 'read_app_state_command') return { timezone: 'UTC', default_post_time: defaultPostTime };
    if (cmd === 'save_app_state_command') return null;
    if (cmd === 'set_default_post_time') return null;
    return null;
  });
}

async function openAppTab() {
  render(<SettingsPanel onClose={vi.fn()} />);
  fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
}

describe('SettingsPanel — App tab — default post time — timezone label (§17.4)', () => {
  it('shows timezone offset label when a timezone is configured', async () => {
    setupMocks(null);
    render(
      <TimezoneContext.Provider value="UTC">
        <SettingsPanel onClose={vi.fn()} />
      </TimezoneContext.Provider>
    );
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    await screen.findByRole('combobox', { name: /default post time hour/i });
    expect(screen.getByText(/\(gmt/i)).toBeInTheDocument();
  });

  it('does not show timezone label when no timezone is configured', async () => {
    setupMocks(null);
    render(
      <TimezoneContext.Provider value="">
        <SettingsPanel onClose={vi.fn()} />
      </TimezoneContext.Provider>
    );
    fireEvent.click(await screen.findByRole('tab', { name: /app/i }));
    await screen.findByRole('combobox', { name: /default post time hour/i });
    expect(screen.queryByText(/\(gmt/i)).not.toBeInTheDocument();
  });
});

describe('SettingsPanel — App tab — default post time — initial state (§17.4)', () => {
  it('renders a "Default post time" label on the App tab', async () => {
    setupMocks(null);
    await openAppTab();
    expect(await screen.findByText(/default post time/i)).toBeInTheDocument();
  });

  it('renders hour and minute selects', async () => {
    setupMocks(null);
    await openAppTab();
    expect(await screen.findByRole('combobox', { name: /default post time hour/i })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: /default post time minute/i })).toBeInTheDocument();
  });

  it('hour select shows placeholder when default_post_time is null', async () => {
    setupMocks(null);
    await openAppTab();
    const hourSelect = await screen.findByRole('combobox', { name: /default post time hour/i });
    expect((hourSelect as HTMLSelectElement).value).toBe('');
  });

  it('hour select shows the saved hour when default_post_time is set', async () => {
    setupMocks({ hour: 9, minute: 30 });
    await openAppTab();
    const hourSelect = await screen.findByRole('combobox', { name: /default post time hour/i });
    expect((hourSelect as HTMLSelectElement).value).toBe('9');
  });

  it('minute select shows the saved minute when default_post_time is set', async () => {
    setupMocks({ hour: 9, minute: 30 });
    await openAppTab();
    const minuteSelect = await screen.findByRole('combobox', { name: /default post time minute/i });
    expect((minuteSelect as HTMLSelectElement).value).toBe('30');
  });
});

describe('SettingsPanel — App tab — default post time — interactions (§17.4)', () => {
  it('changing the hour select calls set_default_post_time with the new hour', async () => {
    setupMocks(null);
    await openAppTab();
    const hourSelect = await screen.findByRole('combobox', { name: /default post time hour/i });
    fireEvent.change(hourSelect, { target: { value: '14' } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_default_post_time',
        expect.objectContaining({ dpt: expect.objectContaining({ hour: 14 }) }),
      ),
    );
  });

  it('changing the minute select calls set_default_post_time with the new minute', async () => {
    setupMocks({ hour: 9, minute: 0 });
    await openAppTab();
    const minuteSelect = await screen.findByRole('combobox', { name: /default post time minute/i });
    fireEvent.change(minuteSelect, { target: { value: '45' } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_default_post_time',
        expect.objectContaining({ dpt: expect.objectContaining({ minute: 45 }) }),
      ),
    );
  });

  it('does not show a Clear button when default_post_time is null', async () => {
    setupMocks(null);
    await openAppTab();
    await screen.findByRole('combobox', { name: /default post time hour/i });
    expect(screen.queryByRole('button', { name: /clear default post time/i })).not.toBeInTheDocument();
  });

  it('shows a Clear button when default_post_time is set', async () => {
    setupMocks({ hour: 9, minute: 30 });
    await openAppTab();
    expect(await screen.findByRole('button', { name: /clear default post time/i })).toBeInTheDocument();
  });

  it('clicking Clear calls set_default_post_time with null', async () => {
    setupMocks({ hour: 9, minute: 30 });
    await openAppTab();
    const clearBtn = await screen.findByRole('button', { name: /clear default post time/i });
    fireEvent.click(clearBtn);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_default_post_time', { dpt: null }),
    );
  });
});
