// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

import SchedulerConnect from './SchedulerConnect';

const defaultProps = {
  workspaceId: 'ws-1',
  provider: 'zernio' as const,
  onSuccess: vi.fn(),
  onCancel: vi.fn(),
};

beforeEach(() => { vi.clearAllMocks(); });

describe('SchedulerConnect — key entry phase', () => {
  it('test_renders_api_key_input', () => {
    render(<SchedulerConnect {...defaultProps} />);
    expect(screen.getByRole('textbox')).toBeDefined();
    expect(screen.getByRole('button', { name: /connect/i })).toBeDefined();
  });

  it('test_returns_to_key_entry_on_connection_failure', async () => {
    mockInvoke.mockRejectedValue(new Error('invalid key'));
    render(<SchedulerConnect {...defaultProps} />);
    await userEvent.type(screen.getByRole('textbox'), 'bad-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => {
      expect(screen.getByRole('textbox')).toBeDefined();
      expect(screen.getByRole('alert')).toBeDefined();
    });
  });

  it('test_calls_onCancel', async () => {
    const onCancel = vi.fn();
    render(<SchedulerConnect {...defaultProps} onCancel={onCancel} />);
    await userEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalledOnce();
  });
});

describe('SchedulerConnect — connection and profiles', () => {
  it('test_connect_calls_test_scheduler_connection', async () => {
    mockInvoke.mockResolvedValue({ profiles: [{ id: 'p1', name: 'My Twitter' }] });
    render(<SchedulerConnect {...defaultProps} />);
    await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('test_scheduler_connection', {
        provider: 'zernio', apiKey: 'my-api-key', workspaceId: 'ws-1',
      });
    });
  });

  it('test_shows_profiles_after_connection_success', async () => {
    mockInvoke.mockResolvedValue({ profiles: [{ id: 'p1', name: 'My Twitter' }, { id: 'p2', name: 'My LinkedIn' }] });
    render(<SchedulerConnect {...defaultProps} />);
    await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => {
      expect(screen.getByText('My Twitter')).toBeDefined();
      expect(screen.getByText('My LinkedIn')).toBeDefined();
    });
  });

  it('test_save_calls_save_scheduler_profiles', async () => {
    mockInvoke
      .mockResolvedValueOnce({ profiles: [{ id: 'p1', name: 'My Twitter' }] })
      .mockResolvedValueOnce(undefined);
    render(<SchedulerConnect {...defaultProps} />);
    await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => screen.getByRole('button', { name: /save/i }));
    await userEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_scheduler_profiles', {
        workspaceId: 'ws-1', provider: 'zernio', selectedProfiles: ['p1'],
      });
    });
  });

  it('test_calls_onSuccess_after_save', async () => {
    const onSuccess = vi.fn();
    mockInvoke
      .mockResolvedValueOnce({ profiles: [{ id: 'p1', name: 'My Twitter' }] })
      .mockResolvedValueOnce(undefined);
    render(<SchedulerConnect {...defaultProps} onSuccess={onSuccess} />);
    await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => screen.getByRole('button', { name: /save/i }));
    await userEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => expect(onSuccess).toHaveBeenCalledWith('zernio'));
  });
});
