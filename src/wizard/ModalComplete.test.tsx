// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

import ModalComplete from './ModalComplete';

const defaultProps = {
  schedulerLinked: false,
  onComplete: vi.fn(),
  onBack: vi.fn(),
};

beforeEach(() => { vi.clearAllMocks(); mockInvoke.mockResolvedValue(undefined); });

describe('ModalComplete', () => {
  it('test_renders_continue_button', () => {
    render(<ModalComplete {...defaultProps} />);
    expect(screen.getByRole('button', { name: /continue/i })).toBeDefined();
  });

  it('test_continue_invokes_set_wizard_completed', async () => {
    render(<ModalComplete {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /continue/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_wizard_completed'));
  });

  it('test_continue_calls_onComplete', async () => {
    const onComplete = vi.fn();
    render(<ModalComplete {...defaultProps} onComplete={onComplete} />);
    await userEvent.click(screen.getByRole('button', { name: /continue/i }));
    await waitFor(() => expect(onComplete).toHaveBeenCalledOnce());
  });

  it('test_shows_scheduler_connected_badge_when_linked', () => {
    render(<ModalComplete {...defaultProps} schedulerLinked={true} />);
    expect(screen.getByText(/scheduler connected/i)).toBeDefined();
  });

  it('test_hides_scheduler_connected_badge_when_not_linked', () => {
    render(<ModalComplete {...defaultProps} schedulerLinked={false} />);
    expect(screen.queryByText(/scheduler connected/i)).toBeNull();
  });

  it('test_back_calls_onBack', async () => {
    const onBack = vi.fn();
    render(<ModalComplete {...defaultProps} onBack={onBack} />);
    await userEvent.click(screen.getByRole('button', { name: /back/i }));
    expect(onBack).toHaveBeenCalledOnce();
  });
});
