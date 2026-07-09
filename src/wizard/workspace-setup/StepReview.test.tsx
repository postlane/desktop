// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MantineProvider } from '@mantine/core';
import '@testing-library/jest-dom';

vi.mock('../../ipc/invoke', () => ({ invoke: vi.fn() }));

import { invoke } from '../../ipc/invoke';
import StepReview from './StepReview';
import type { ChildRepo, WorkspaceConfig } from './types';

const mockInvoke = vi.mocked(invoke);

const config: WorkspaceConfig = {
  project_id: 'proj-1',
  base_url: 'https://postlane.dev',
  platforms: ['x', 'bluesky'],
  mastodon_instance: null,
  llm_provider: 'anthropic',
  llm_model: 'claude-sonnet-4-6',
  author: 'Jordan Reyes',
  style: 'Direct, no jargon',
  utm_campaign: null,
  attribution: true,
  scheduler_provider: 'zernio',
  scheduler_api_key: 'zk_live_secret',
  scheduler_profile_id: null,
};

const childRepos: ChildRepo[] = [
  { name: 'frontend', path: '/Users/jordan/code/myorg/frontend', posts_dir: 'frontend' },
];

function renderStep(onBack = vi.fn(), onComplete = vi.fn(), onUpgradeClick = vi.fn()) {
  render(
    <MantineProvider>
      <StepReview
        workspacePath="/Users/jordan/code/myorg"
        childRepos={childRepos}
        config={config}
        onBack={onBack}
        onComplete={onComplete}
        onUpgradeClick={onUpgradeClick}
      />
    </MantineProvider>,
  );
  return { onBack, onComplete, onUpgradeClick };
}

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('StepReview — rendering', () => {
  it('renders the config summary and discovered repo list', () => {
    renderStep();
    expect(screen.getByText('Jordan Reyes')).toBeInTheDocument();
    expect(screen.getByText('Direct, no jargon')).toBeInTheDocument();
    expect(screen.getByText('frontend')).toBeInTheDocument();
  });
});

describe('StepReview — setup_workspace submission (checklist 24.3.6)', () => {
  it('calls setup_workspace with the full config on submit', async () => {
    mockInvoke.mockResolvedValue(null);
    renderStep();
    fireEvent.click(screen.getByRole('button', { name: /set up workspace/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('setup_workspace', {
        path: '/Users/jordan/code/myorg',
        config,
        childRepos,
      });
    });
  });

  it('disables the submit button while in flight and does not double-submit', async () => {
    let resolveSetup: (v: unknown) => void = () => {};
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'setup_workspace') return new Promise((resolve) => { resolveSetup = resolve; });
      return Promise.resolve({ status: 'free_owned' });
    });
    renderStep();
    const button = screen.getByRole('button', { name: /set up workspace/i });
    fireEvent.click(button);
    expect(button).toBeDisabled();
    fireEvent.click(button); // second click while pending must not re-invoke
    resolveSetup(null);
    await waitFor(() => expect(screen.getByText(/workspace connected/i)).toBeInTheDocument());
    expect(mockInvoke).toHaveBeenCalledTimes(2); // setup_workspace + get_workspace_billing_status, exactly once each
  });

  it('shows an inline error and stays mounted on setup_workspace failure', async () => {
    mockInvoke.mockRejectedValue('disk full');
    const { onComplete } = renderStep();
    fireEvent.click(screen.getByRole('button', { name: /set up workspace/i }));
    await waitFor(() => expect(screen.getByText('disk full')).toBeInTheDocument());
    expect(onComplete).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: /set up workspace/i })).not.toBeDisabled();
  });

  it('shows the success message on success', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'setup_workspace') return Promise.resolve(null);
      return Promise.resolve({ status: 'free_owned' });
    });
    renderStep();
    fireEvent.click(screen.getByRole('button', { name: /set up workspace/i }));
    await waitFor(() => {
      expect(
        screen.getByText('Workspace connected. Invoke /draft-post in your IDE to create your first post.'),
      ).toBeInTheDocument();
    });
  });
});

describe('StepReview — billing-status integration (checklist 24.3.6a)', () => {
  it('calls billing-status only after setup_workspace resolves', async () => {
    const order: string[] = [];
    mockInvoke.mockImplementation((cmd) => {
      order.push(String(cmd));
      if (cmd === 'setup_workspace') return Promise.resolve(null);
      return Promise.resolve({ status: 'free_owned' });
    });
    renderStep();
    fireEvent.click(screen.getByRole('button', { name: /set up workspace/i }));
    await waitFor(() => expect(order).toEqual(['setup_workspace', 'get_workspace_billing_status']));
    expect(mockInvoke).toHaveBeenLastCalledWith('get_workspace_billing_status', { projectId: 'proj-1' });
  });

  it('shows the paid_required banner alongside the success message, wires onUpgradeClick', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'setup_workspace') return Promise.resolve(null);
      return Promise.resolve({ status: 'paid_required' });
    });
    const { onUpgradeClick } = renderStep();
    fireEvent.click(screen.getByRole('button', { name: /set up workspace/i }));
    await waitFor(() => {
      expect(
        screen.getByText('Workspace connected. Invoke /draft-post in your IDE to create your first post.'),
      ).toBeInTheDocument();
      expect(screen.getByText(/this is your second workspace/i)).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: /add to plan/i }));
    expect(onUpgradeClick).toHaveBeenCalled();
  });

  // Telemetry itself fires server-side inside get_workspace_billing_status's
  // Rust command body (checklist 24.3.6a) so it can't be skipped by a
  // frontend bug -- from the frontend's side, this is the same invoke
  // assertion as the ordering test above, just asserting the mechanism
  // that triggers it was actually reached for a paid_required workspace.
  it('reaches get_workspace_billing_status for a paid_required workspace (fires workspace_upgrade_prompted server-side)', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'setup_workspace') return Promise.resolve(null);
      return Promise.resolve({ status: 'paid_required' });
    });
    renderStep();
    fireEvent.click(screen.getByRole('button', { name: /set up workspace/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_workspace_billing_status', { projectId: 'proj-1' });
    });
  });

  it('does not block success on a billing-status failure', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'setup_workspace') return Promise.resolve(null);
      return Promise.reject('network error');
    });
    renderStep();
    fireEvent.click(screen.getByRole('button', { name: /set up workspace/i }));
    await waitFor(() => {
      expect(
        screen.getByText('Workspace connected. Invoke /draft-post in your IDE to create your first post.'),
      ).toBeInTheDocument();
    });
  });
});

describe('StepReview — Continue after success', () => {
  it('clears wizard state and calls onComplete when Continue is clicked', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'setup_workspace') return Promise.resolve(null);
      if (cmd === 'get_workspace_billing_status') return Promise.resolve({ status: 'free_owned' });
      return Promise.resolve(null);
    });
    const { onComplete } = renderStep();
    fireEvent.click(screen.getByRole('button', { name: /set up workspace/i }));
    await waitFor(() => screen.getByRole('button', { name: /continue/i }));
    fireEvent.click(screen.getByRole('button', { name: /continue/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('clear_wizard_state');
      expect(mockInvoke).toHaveBeenCalledWith('set_wizard_completed');
      expect(onComplete).toHaveBeenCalled();
    });
  });
});
