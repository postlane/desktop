// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

import ModalComplete from './ModalComplete';

const defaultProps = {
  schedulerLinked: false,
  repoConnected: false,
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

// ── repo connected text variants ──────────────────────────────────────────────

describe('ModalComplete — repo connected text variants', () => {
  it('test_no_repo_subtitle_mentions_add_repos', () => {
    render(<ModalComplete {...defaultProps} repoConnected={false} />);
    expect(screen.getByText(/add repos from the dashboard/i)).toBeDefined();
  });

  it('test_repo_connected_subtitle_says_ready_to_start_drafting', () => {
    render(<ModalComplete {...defaultProps} repoConnected={true} />);
    expect(screen.getByText(/ready to start drafting/i)).toBeDefined();
  });

  it('test_no_repo_body_tells_user_to_add_a_repo_first', () => {
    render(<ModalComplete {...defaultProps} repoConnected={false} />);
    expect(screen.getByText(/add a repo/i)).toBeDefined();
  });

  it('test_repo_connected_body_links_to_documentation', () => {
    render(<ModalComplete {...defaultProps} repoConnected={true} />);
    expect(screen.getByRole('link', { name: /documentation/i })).toBeDefined();
  });
});
