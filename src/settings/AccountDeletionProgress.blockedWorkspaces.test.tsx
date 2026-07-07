// SPDX-License-Identifier: BUSL-1.1
// Tests for checklist 24.4.15a — account-deletion 409 resolution UI.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { MantineProvider } from '@mantine/core';
import AccountDeletionProgress from './AccountDeletionProgress';

const mockInvoke = vi.fn();
vi.mock('../ipc/invoke', () => ({ invoke: (...a: unknown[]) => mockInvoke(...a) }));

function renderProgress(props: { deleteWorkspaceDirs: boolean; onDeleted: () => void; onAbort: () => void }) {
  return render(
    <MantineProvider>
      <AccountDeletionProgress {...props} />
    </MantineProvider>,
  );
}

interface PhaseResult { phase: number; message: string; next_phase: number | null; }

function phaseOk(phase: number, next: number | null = null): PhaseResult {
  return { phase, message: 'ok', next_phase: next };
}

const BLOCKED_ERROR = {
  phase: 5,
  code: 'PL-DEL-BLOCKED',
  message: 'This account owns a workspace with active collaborators.',
  skippable: false,
  blocked_workspaces: [
    {
      project_id: 'proj-1',
      admin_collaborators: [{ user_id: 'admin-1', display_name: 'Ada Lovelace' }],
    },
  ],
};

const defaultProps = { deleteWorkspaceDirs: false, onDeleted: vi.fn(), onAbort: vi.fn() };

function setupThroughPhase4ThenBlocked(finalError: typeof BLOCKED_ERROR = BLOCKED_ERROR) {
  mockInvoke.mockImplementation((cmd: string, args?: { phase: number }) => {
    if (cmd === 'sign_out') return Promise.resolve();
    if (cmd === 'run_deletion_phase' && args !== undefined) {
      if (args.phase < 5) return Promise.resolve(phaseOk(args.phase, args.phase + 1));
      return Promise.reject(finalError);
    }
    return Promise.resolve();
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  defaultProps.onDeleted = vi.fn();
  defaultProps.onAbort = vi.fn();
});

describe('24.4.15a: blocked-workspaces resolution panel — rendering', () => {
  it('renders a transfer picker when the workspace has eligible admin collaborators', async () => {
    setupThroughPhase4ThenBlocked();
    renderProgress(defaultProps);
    await waitFor(() => expect(screen.getByText(/ada lovelace/i)).toBeInTheDocument());
    expect(screen.getByRole('button', { name: /transfer to/i })).toBeInTheDocument();
  });

  it('shows "Start 14-day departure window" for every blocked workspace', async () => {
    setupThroughPhase4ThenBlocked();
    renderProgress(defaultProps);
    await waitFor(() => expect(screen.getByRole('button', { name: /start 14-day departure window/i })).toBeInTheDocument());
  });

  it('shows "Promote a collaborator to admin first" when there are zero eligible admins', async () => {
    setupThroughPhase4ThenBlocked({
      ...BLOCKED_ERROR,
      blocked_workspaces: [{ project_id: 'proj-1', admin_collaborators: [] }],
    });
    renderProgress(defaultProps);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /promote a collaborator to admin first/i })).toBeInTheDocument(),
    );
    expect(screen.queryByRole('button', { name: /^transfer to/i })).not.toBeInTheDocument();
  });

  it('clicking "Promote a collaborator to admin first" calls onAbort', async () => {
    setupThroughPhase4ThenBlocked({
      ...BLOCKED_ERROR,
      blocked_workspaces: [{ project_id: 'proj-1', admin_collaborators: [] }],
    });
    const onAbort = vi.fn();
    renderProgress({ ...defaultProps, onAbort });
    await waitFor(() => screen.getByRole('button', { name: /promote a collaborator to admin first/i }));
    fireEvent.click(screen.getByRole('button', { name: /promote a collaborator to admin first/i }));
    expect(onAbort).toHaveBeenCalled();
  });

  it('does not show the generic ErrorPanel Retry/Abort buttons for a blocked 409', async () => {
    setupThroughPhase4ThenBlocked();
    renderProgress(defaultProps);
    await waitFor(() => expect(screen.getByRole('button', { name: /start 14-day departure window/i })).toBeInTheDocument());
    expect(screen.queryByRole('button', { name: /^retry$/i })).not.toBeInTheDocument();
  });
});

describe('24.4.15a: blocked-workspaces resolution panel — resolving', () => {
  it('transferring to the selected admin calls transfer_workspace_to_admin and retries deletion once resolved', async () => {
    setupThroughPhase4ThenBlocked();
    renderProgress(defaultProps);
    await waitFor(() => screen.getByRole('button', { name: /transfer to/i }));

    fireEvent.change(screen.getByRole('combobox', { name: /choose an admin collaborator/i }), {
      target: { value: 'admin-1' },
    });

    mockInvoke.mockImplementation((cmd: string, args?: { phase: number }) => {
      if (cmd === 'transfer_workspace_to_admin') return Promise.resolve();
      if (cmd === 'sign_out') return Promise.resolve();
      if (cmd === 'run_deletion_phase' && args !== undefined) {
        return Promise.resolve(phaseOk(args.phase, args.phase < 7 ? args.phase + 1 : null));
      }
      return Promise.resolve();
    });
    fireEvent.click(screen.getByRole('button', { name: /transfer to/i }));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('transfer_workspace_to_admin', {
      projectId: 'proj-1', targetUserId: 'admin-1',
    }));
    await waitFor(() => expect(screen.queryByText(/account has been deleted/i)).toBeInTheDocument());
  });

  it('starting the departure window calls initiate_ownership_departure and retries deletion once resolved', async () => {
    setupThroughPhase4ThenBlocked();
    renderProgress(defaultProps);
    await waitFor(() => screen.getByRole('button', { name: /start 14-day departure window/i }));

    mockInvoke.mockImplementation((cmd: string, args?: { phase: number }) => {
      if (cmd === 'initiate_ownership_departure') return Promise.resolve();
      if (cmd === 'sign_out') return Promise.resolve();
      if (cmd === 'run_deletion_phase' && args !== undefined) {
        return Promise.resolve(phaseOk(args.phase, args.phase < 7 ? args.phase + 1 : null));
      }
      return Promise.resolve();
    });
    fireEvent.click(screen.getByRole('button', { name: /start 14-day departure window/i }));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('initiate_ownership_departure', { projectId: 'proj-1' }));
    await waitFor(() => expect(screen.queryByText(/account has been deleted/i)).toBeInTheDocument());
  });
});

describe('24.4.15a: blocked-workspaces resolution panel — multi-workspace and errors', () => {
  it('shows multiple blocked workspaces and only retries deletion once all are resolved', async () => {
    setupThroughPhase4ThenBlocked({
      ...BLOCKED_ERROR,
      blocked_workspaces: [
        { project_id: 'proj-1', admin_collaborators: [] },
        { project_id: 'proj-2', admin_collaborators: [] },
      ],
    });
    renderProgress(defaultProps);
    await waitFor(() => expect(screen.getAllByRole('button', { name: /start 14-day departure window/i })).toHaveLength(2));

    mockInvoke.mockImplementation((cmd: string, args?: { phase: number }) => {
      if (cmd === 'initiate_ownership_departure') return Promise.resolve();
      if (cmd === 'sign_out') return Promise.resolve();
      if (cmd === 'run_deletion_phase' && args !== undefined) {
        return Promise.resolve(phaseOk(args.phase, args.phase < 7 ? args.phase + 1 : null));
      }
      return Promise.resolve();
    });

    fireEvent.click(screen.getAllByRole('button', { name: /start 14-day departure window/i })[0]);
    await waitFor(() => expect(screen.getAllByRole('button', { name: /start 14-day departure window/i })).toHaveLength(1));
    // resolving only one of two workspaces must not yet retry deletion (phase 5 called only once, from the initial attempt)
    const phase5Calls = mockInvoke.mock.calls.filter(
      ([cmd, args]) => cmd === 'run_deletion_phase' && (args as { phase: number }).phase === 5,
    );
    expect(phase5Calls).toHaveLength(1);

    fireEvent.click(screen.getByRole('button', { name: /start 14-day departure window/i }));
    await waitFor(() => expect(screen.queryByText(/account has been deleted/i)).toBeInTheDocument());
  });

  it('shows an action error and does not resolve the workspace when the transfer fails', async () => {
    setupThroughPhase4ThenBlocked();
    renderProgress(defaultProps);
    await waitFor(() => screen.getByRole('button', { name: /start 14-day departure window/i }));

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'initiate_ownership_departure') return Promise.reject(new Error('forbidden'));
      return Promise.resolve();
    });
    fireEvent.click(screen.getByRole('button', { name: /start 14-day departure window/i }));

    await waitFor(() => expect(screen.getByText('forbidden')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: /start 14-day departure window/i })).toBeInTheDocument();
  });
});
