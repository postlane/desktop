// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
const mockWriteText = vi.fn().mockResolvedValue(undefined);
vi.stubGlobal('navigator', { clipboard: { writeText: mockWriteText } });

import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

import ModalRepository from './ModalRepository';

const defaultProps = {
  workspaceId: 'ws-1',
  onBack: vi.fn(),
  onComplete: vi.fn(),
  pollIntervalMs: 10000,
};

beforeEach(() => { vi.clearAllMocks(); });

describe('ModalRepository — detecting phase', () => {
  it('test_renders_cli_command_block', () => {
    mockInvoke.mockResolvedValue([]);
    render(<ModalRepository {...defaultProps} />);
    expect(screen.getByText(/npx @postlane\/cli init/)).toBeDefined();
  });

  it('test_copy_button_copies_command', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<ModalRepository {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /copy/i }));
    expect(mockWriteText).toHaveBeenCalledWith('npx @postlane/cli init');
  });

  it('test_poll_clears_on_unmount', () => {
    mockInvoke.mockResolvedValue([]);
    const clearSpy = vi.spyOn(globalThis, 'clearInterval');
    const { unmount } = render(<ModalRepository {...defaultProps} pollIntervalMs={10000} />);
    unmount();
    expect(clearSpy).toHaveBeenCalled();
    clearSpy.mockRestore();
  });
});

describe('ModalRepository — done phase', () => {
  it('test_detects_repo_and_transitions_to_done', async () => {
    mockInvoke.mockResolvedValueOnce([]).mockResolvedValueOnce(['my-repo']).mockResolvedValue(undefined);
    render(<ModalRepository {...defaultProps} pollIntervalMs={30} />);
    await waitFor(() => { expect(screen.getByText(/my-repo/i)).toBeDefined(); }, { timeout: 3000 });
  });

  it('test_commit_notice_shown_after_registration', async () => {
    mockInvoke.mockResolvedValueOnce([]).mockResolvedValueOnce(['my-repo']).mockResolvedValue(undefined);
    render(<ModalRepository {...defaultProps} pollIntervalMs={30} />);
    await waitFor(() => { expect(screen.getByText(/commit/i)).toBeDefined(); }, { timeout: 3000 });
  });

  it('test_open_dashboard_calls_set_wizard_completed', async () => {
    const onComplete = vi.fn();
    mockInvoke.mockResolvedValueOnce([]).mockResolvedValueOnce(['my-repo']).mockResolvedValue(undefined);
    render(<ModalRepository {...defaultProps} onComplete={onComplete} pollIntervalMs={30} />);
    await waitFor(() => screen.getByRole('button', { name: /open dashboard/i }), { timeout: 3000 });
    await userEvent.click(screen.getByRole('button', { name: /open dashboard/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_wizard_completed');
      expect(onComplete).toHaveBeenCalledOnce();
    });
  });

  it('test_add_another_repo_restarts_detection', async () => {
    mockInvoke.mockResolvedValueOnce([]).mockResolvedValueOnce(['my-repo']).mockResolvedValue(undefined);
    render(<ModalRepository {...defaultProps} pollIntervalMs={30} />);
    await waitFor(() => screen.getByRole('button', { name: /add another repo/i }), { timeout: 3000 });
    await userEvent.click(screen.getByRole('button', { name: /add another repo/i }));
    expect(screen.getByText(/npx @postlane\/cli init/)).toBeDefined();
  });
});
