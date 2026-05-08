// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

import ModalScheduler from './ModalScheduler';

const defaultProps = {
  workspaceId: 'ws-1',
  onNext: vi.fn(),
  onBack: vi.fn(),
  setSchedulerLinked: vi.fn(),
};

beforeEach(() => { vi.clearAllMocks(); });

describe('ModalScheduler — picker', () => {
  it('test_renders_provider_options_and_skip', () => {
    render(<ModalScheduler {...defaultProps} />);
    expect(screen.getByRole('button', { name: /zernio/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /publer/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /skip/i })).toBeDefined();
  });

  it('test_selecting_zernio_opens_key_entry', async () => {
    render(<ModalScheduler {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    expect(screen.getByRole('textbox')).toBeDefined();
  });

  it('test_selecting_publer_opens_key_entry', async () => {
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

  it('test_cancel_in_key_entry_returns_to_picker', async () => {
    render(<ModalScheduler {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    await userEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(screen.queryByRole('textbox')).toBeNull();
    expect(screen.getByRole('button', { name: /zernio/i })).toBeDefined();
  });
});

describe('ModalScheduler — after connecting first provider', () => {
  async function connectZernio() {
    mockInvoke.mockResolvedValue(undefined);
    render(<ModalScheduler {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull());
  }

  it('test_stays_on_picker_without_advancing', async () => {
    const onNext = vi.fn();
    mockInvoke.mockResolvedValue(undefined);
    render(<ModalScheduler {...defaultProps} onNext={onNext} />);
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull());
    expect(onNext).not.toHaveBeenCalled();
  });

  it('test_connected_provider_button_is_disabled', async () => {
    await connectZernio();
    expect((screen.getByRole('button', { name: /zernio/i }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('test_skip_is_hidden', async () => {
    await connectZernio();
    expect(screen.queryByRole('button', { name: /skip/i })).toBeNull();
  });

  it('test_next_button_is_visible', async () => {
    await connectZernio();
    expect(screen.getByRole('button', { name: /next/i })).toBeDefined();
  });

  it('test_next_button_calls_onNext', async () => {
    const onNext = vi.fn();
    mockInvoke.mockResolvedValue(undefined);
    render(<ModalScheduler {...defaultProps} onNext={onNext} />);
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull());
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('test_sets_scheduler_linked_true', async () => {
    const setSchedulerLinked = vi.fn();
    mockInvoke.mockResolvedValue(undefined);
    render(<ModalScheduler {...defaultProps} setSchedulerLinked={setSchedulerLinked} />);
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
    await userEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(setSchedulerLinked).toHaveBeenCalledWith(true));
  });

  it('test_second_provider_button_remains_enabled', async () => {
    await connectZernio();
    expect((screen.getByRole('button', { name: /publer/i }) as HTMLButtonElement).disabled).toBe(false);
  });
});
