// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

import { invoke } from '../ipc/invoke';
import Wizard from './Wizard';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'list_provider_orgs') return [];
    return undefined;
  });
});

describe('Wizard', () => {
  it('test_renders_modal_welcome_on_step_1', () => {
    render(<Wizard onComplete={vi.fn()} />);
    expect(screen.getByText('Welcome to Postlane')).toBeDefined();
  });

  it('test_renders_modal_account_on_step_2', () => {
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    expect(screen.getByText('Sign in to Postlane')).toBeDefined();
  });

  // 20.6.3: step 5 now renders GitHub App install, not Complete
  it('test_renders_modal_github_app_on_step_5', () => {
    render(<Wizard onComplete={vi.fn()} startAt={5} />);
    expect(screen.getByRole('button', { name: /install github app/i })).toBeDefined();
  });

  it('step 3 shows a Skip button after orgs load', async () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    await waitFor(() => expect(screen.getByRole('button', { name: /^skip$/i })).toBeDefined());
  });

  it('clicking Skip on step 3 marks wizard complete and calls onComplete', async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} startAt={3} />);
    await waitFor(() => screen.getByRole('button', { name: /^skip$/i }));
    fireEvent.click(screen.getByRole('button', { name: /^skip$/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_wizard_completed'));
    expect(onComplete).toHaveBeenCalled();
  });

  it('step 4 shows a Skip button', () => {
    render(<Wizard onComplete={vi.fn()} startAt={4} />);
    expect(screen.getByRole('button', { name: /^skip$/i })).toBeDefined();
  });

  it('clicking Skip on step 4 marks wizard complete and calls onComplete', async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} startAt={4} />);
    fireEvent.click(screen.getByRole('button', { name: /^skip$/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_wizard_completed'));
    expect(onComplete).toHaveBeenCalled();
  });

  it('test_step_5_skip_button_exists', () => {
    render(<Wizard onComplete={vi.fn()} startAt={5} />);
    expect(screen.getByRole('button', { name: /skip/i })).toBeDefined();
  });

  it('clicking Skip on step 5 marks wizard complete and calls onComplete', async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} startAt={5} />);
    fireEvent.click(screen.getByRole('button', { name: /skip/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_wizard_completed'));
    expect(onComplete).toHaveBeenCalled();
  });

});
