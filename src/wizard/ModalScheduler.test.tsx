// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import ModalScheduler from './ModalScheduler';

const defaultProps = {
  workspaceId: 'ws-1',
  onNext: vi.fn(),
  onBack: vi.fn(),
  setSchedulerLinked: vi.fn(),
};

beforeEach(() => { vi.clearAllMocks(); });

describe('ModalScheduler', () => {
  it('test_renders_three_provider_options', () => {
    render(<ModalScheduler {...defaultProps} />);
    expect(screen.getByText(/zernio/i)).toBeDefined();
    expect(screen.getByText(/publer/i)).toBeDefined();
    expect(screen.getByRole('button', { name: /skip/i })).toBeDefined();
  });

  it('test_selecting_zernio_opens_scheduler_connect', async () => {
    render(<ModalScheduler {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    expect(screen.getByRole('textbox')).toBeDefined();
  });

  it('test_selecting_publer_opens_scheduler_connect', async () => {
    render(<ModalScheduler {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /publer/i }));
    expect(screen.getByRole('textbox')).toBeDefined();
  });

  it('test_skip_calls_onNext_with_scheduler_not_linked', async () => {
    const setSchedulerLinked = vi.fn();
    const onNext = vi.fn();
    render(<ModalScheduler {...defaultProps} setSchedulerLinked={setSchedulerLinked} onNext={onNext} />);
    await userEvent.click(screen.getByRole('button', { name: /skip/i }));
    expect(setSchedulerLinked).toHaveBeenCalledWith(false);
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('test_success_calls_onNext_with_scheduler_linked', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    const mockInvoke = vi.mocked(invoke);
    mockInvoke
      .mockResolvedValueOnce({ profiles: [{ id: 'p1', name: 'My Twitter' }] })
      .mockResolvedValueOnce(undefined);
    const setSchedulerLinked = vi.fn();
    const onNext = vi.fn();
    render(<ModalScheduler {...defaultProps} setSchedulerLinked={setSchedulerLinked} onNext={onNext} />);
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => screen.getByRole('button', { name: /save/i }));
    await userEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => {
      expect(setSchedulerLinked).toHaveBeenCalledWith(true);
      expect(onNext).toHaveBeenCalledOnce();
    });
  });

  it('test_back_from_subflow_returns_to_provider_cards', async () => {
    render(<ModalScheduler {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    expect(screen.getByRole('textbox')).toBeDefined();
    await userEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(screen.queryByRole('textbox')).toBeNull();
    expect(screen.getByText(/zernio/i)).toBeDefined();
  });
});
