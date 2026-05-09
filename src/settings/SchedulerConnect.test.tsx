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

describe('SchedulerConnect — key entry', () => {
  it('test_renders_api_key_input', () => {
    render(<SchedulerConnect {...defaultProps} />);
    expect(screen.getByRole('textbox')).toBeDefined();
    expect(screen.getByRole('button', { name: /connect/i })).toBeDefined();
  });

  it('test_connect_calls_save_scheduler_credential', async () => {
    mockInvoke.mockResolvedValue(undefined);
    render(<SchedulerConnect {...defaultProps} />);
    await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_scheduler_credential', {
        provider: 'zernio', apiKey: 'my-api-key', repoId: null,
      });
    });
  });

  it('test_calls_onSuccess_on_connect_success', async () => {
    const onSuccess = vi.fn();
    mockInvoke.mockResolvedValue(undefined);
    render(<SchedulerConnect {...defaultProps} onSuccess={onSuccess} />);
    await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(onSuccess).toHaveBeenCalledWith('zernio'));
  });

  it('test_shows_error_on_connect_failure', async () => {
    mockInvoke.mockRejectedValue(new Error('invalid key'));
    render(<SchedulerConnect {...defaultProps} />);
    await userEvent.type(screen.getByRole('textbox'), 'bad-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeDefined();
      expect(screen.getByRole('textbox')).toBeDefined();
    });
  });

  it('test_calls_onCancel', async () => {
    const onCancel = vi.fn();
    render(<SchedulerConnect {...defaultProps} onCancel={onCancel} />);
    await userEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalledOnce();
  });
});
