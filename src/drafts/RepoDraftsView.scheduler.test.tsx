// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import RepoDraftsView from './RepoDraftsView';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

function setupInvoke(schedulerConfigured: boolean) {
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'get_all_drafts') return [];
    if (cmd === 'has_scheduler_configured') return schedulerConfigured;
    return null;
  });
}

// §13.4.5 — scheduler setup modal is shown when the repo has no scheduler configured
describe('RepoDraftsView — scheduler onboarding', () => {
  it('shows the scheduler setup modal when no scheduler is configured', async () => {
    setupInvoke(false);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByRole('dialog')).toBeInTheDocument(),
    );
  });

  it('does not show the modal when a scheduler is already configured', async () => {
    setupInvoke(true);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() =>
      expect(screen.getByText(/no drafts waiting/i)).toBeInTheDocument(),
    );
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  // §13.4.6 — "Set up later" dismisses the modal and shows an inline warning
  it('dismisses the modal and shows inline warning on "Set up later"', async () => {
    setupInvoke(false);
    render(<RepoDraftsView repoId="r1" />);
    await waitFor(() => expect(screen.getByRole('dialog')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /set up later/i }));

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(screen.getByText(/no scheduler configured/i)).toBeInTheDocument();
  });
});
