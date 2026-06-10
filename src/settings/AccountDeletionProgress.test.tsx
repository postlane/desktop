// SPDX-License-Identifier: BUSL-1.1
// Tests for §22.7.6/22.7.7 — AccountDeletionProgress step machine.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import AccountDeletionProgress from './AccountDeletionProgress';

const mockInvoke = vi.fn();
vi.mock('../ipc/invoke', () => ({ invoke: (...a: unknown[]) => mockInvoke(...a) }));

interface PhaseResult { phase: number; message: string; next_phase: number | null; }
interface PhaseError { phase: number; code: string; message: string; skippable: boolean; }

function phaseOk(phase: number, next: number | null = null): PhaseResult {
  const msgs: Record<number, string> = {
    0: 'Verifying session…',
    1: 'Removing project data…',
    2: 'Removing project data…',
    3: 'Revoking integrations…',
    4: 'Clearing credentials…',
    5: 'Removing account record…',
    6: 'Cleaning up local files…',
    7: 'Removing workspace files…',
  };
  return { phase, message: msgs[phase] ?? 'Finishing…', next_phase: next };
}

function phaseErr(phase: number, skippable = true): PhaseError {
  return { phase, code: `PL-DEL-00${phase}`, message: 'step failed', skippable };
}

function setupAllSuccess() {
  mockInvoke.mockImplementation((cmd: string, args?: { phase: number }) => {
    if (cmd === 'sign_out' || args === undefined) return Promise.resolve();
    const phase = args.phase;
    return Promise.resolve(phaseOk(phase, phase < 7 ? phase + 1 : null));
  });
}

const defaultProps = { deleteWorkspaceDirs: false, onDeleted: vi.fn(), onAbort: vi.fn() };

beforeEach(() => {
  vi.clearAllMocks();
  defaultProps.onDeleted = vi.fn();
  defaultProps.onAbort = vi.fn();
});

// ── 22.7.6: progress messages ─────────────────────────────────────────────────

describe('22.7.6: progress messages', () => {
  it('shows "Verifying session…" for phase 0', async () => {
    setupAllSuccess();
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() => expect(screen.queryByText(/Verifying session/i)).not.toBeNull());
  });

  it('shows "Removing account record…" when phase 5 runs', async () => {
    // Mock phases 0-4 succeeding, phase 5 pending
    let resolvePhase5: ((v: PhaseResult) => void) | undefined;
    const phase5Promise = new Promise<PhaseResult>((res) => { resolvePhase5 = res; });
    mockInvoke.mockImplementation((_cmd: string, args: { phase: number }) => {
      if (args.phase < 5) return Promise.resolve(phaseOk(args.phase, args.phase + 1));
      return phase5Promise;
    });
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() => expect(screen.queryByText(/Removing account record/i)).not.toBeNull());
    if (resolvePhase5) resolvePhase5(phaseOk(5, 6));
  });

  it('shows completion message after all phases', async () => {
    setupAllSuccess();
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() =>
      expect(screen.queryByText(/account has been deleted/i)).not.toBeNull()
    );
  });

  it('calls onDeleted after completion', async () => {
    setupAllSuccess();
    const onDeleted = vi.fn();
    render(<AccountDeletionProgress {...{ ...defaultProps, onDeleted }} />);
    await waitFor(() => expect(onDeleted).toHaveBeenCalled());
  });
});

// ── 22.7.7 / 22.7.19: step failure shows Retry + Skip ────────────────────────

describe('22.7.7 / 22.7.19: step failure Retry and Skip', () => {
  it('shows error message on phase failure', async () => {
    mockInvoke.mockRejectedValue(phaseErr(1));
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() => expect(screen.queryByText(/step failed/i)).not.toBeNull());
  });

  it('shows Retry button on any phase failure', async () => {
    mockInvoke.mockRejectedValue(phaseErr(1));
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() => expect(screen.queryByRole('button', { name: /Retry/i })).not.toBeNull());
  });

  it('shows Skip button for skippable phase failure (phase 1)', async () => {
    mockInvoke.mockRejectedValue(phaseErr(1, true));
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: /Skip and continue/i })).not.toBeNull()
    );
  });

  it('clicking Retry re-runs the same phase and continues on success', async () => {
    let phase1Calls = 0;
    mockInvoke.mockImplementation((cmd: string, args?: { phase: number }) => {
      if (cmd === 'sign_out' || args === undefined) return Promise.resolve();
      if (args.phase === 1) {
        phase1Calls++;
        if (phase1Calls === 1) return Promise.reject(phaseErr(1));
      }
      return Promise.resolve(phaseOk(args.phase, args.phase < 7 ? args.phase + 1 : null));
    });
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() => screen.getByRole('button', { name: /Retry/i }));
    fireEvent.click(screen.getByRole('button', { name: /Retry/i }));
    await waitFor(() =>
      expect(screen.queryByText(/account has been deleted/i)).not.toBeNull()
    );
    expect(phase1Calls).toBe(2);
  });

  it('clicking Skip advances to next phase', async () => {
    const phases: number[] = [];
    mockInvoke.mockImplementation((_cmd: string, args: { phase: number }) => {
      phases.push(args.phase);
      if (args.phase === 1) return Promise.reject(phaseErr(1, true));
      return Promise.resolve(phaseOk(args.phase, args.phase < 7 ? args.phase + 1 : null));
    });
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() => screen.getByRole('button', { name: /Skip and continue/i }));
    fireEvent.click(screen.getByRole('button', { name: /Skip and continue/i }));
    await waitFor(() => expect(phases).toContain(2));
  });

  it('shows skip warning text', async () => {
    mockInvoke.mockRejectedValue(phaseErr(1, true));
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() =>
      expect(screen.queryByText(/orphaned data/i)).not.toBeNull()
    );
  });
});

// ── 22.7.19a / 22.7.21a: Phase 5 — critical, no skip ────────────────────────

describe('22.7.19a / 22.7.21a: Phase 5 critical — Retry and Abort only', () => {
  async function triggerPhase5Error() {
    mockInvoke.mockImplementation((_cmd: string, args: { phase: number }) => {
      if (args.phase < 5) return Promise.resolve(phaseOk(args.phase, args.phase + 1));
      return Promise.reject(phaseErr(5, false));
    });
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() => screen.getByRole('button', { name: /Retry/i }));
  }

  it('does NOT show Skip button for phase 5 failure', async () => {
    await triggerPhase5Error();
    expect(screen.queryByRole('button', { name: /Skip/i })).toBeNull();
  });

  it('shows Retry button for phase 5 failure', async () => {
    await triggerPhase5Error();
    expect(screen.queryByRole('button', { name: /Retry/i })).not.toBeNull();
  });

  it('shows Abort button for phase 5 failure', async () => {
    await triggerPhase5Error();
    expect(screen.queryByRole('button', { name: /Abort/i })).not.toBeNull();
  });

  it('shows non-skippable message for phase 5', async () => {
    await triggerPhase5Error();
    expect(screen.queryByText(/cannot be skipped/i)).not.toBeNull();
  });

  it('Abort on phase 5 calls onAbort', async () => {
    const onAbort = vi.fn();
    mockInvoke.mockImplementation((_cmd: string, args: { phase: number }) => {
      if (args.phase < 5) return Promise.resolve(phaseOk(args.phase, args.phase + 1));
      return Promise.reject(phaseErr(5, false));
    });
    render(<AccountDeletionProgress {...{ ...defaultProps, onAbort }} />);
    await waitFor(() => screen.getByRole('button', { name: /Abort/i }));
    fireEvent.click(screen.getByRole('button', { name: /Abort/i }));
    expect(onAbort).toHaveBeenCalled();
  });
});

// ── B17: Phase 0 pre-flight failure ──────────────────────────────────────────

describe('B17: Phase 0 pre-flight failure — session expired / no token', () => {
  async function triggerPhase0Error(message = 'Your session has expired. Sign out and sign back in to continue.') {
    mockInvoke.mockImplementation(() =>
      Promise.reject({ phase: 0, code: 'PL-DEL-000', message, skippable: false })
    );
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() => screen.getByRole('button', { name: /Retry/i }));
  }

  it('does NOT show Skip button for phase 0 failure', async () => {
    await triggerPhase0Error();
    expect(screen.queryByRole('button', { name: /Skip/i })).toBeNull();
  });

  it('shows Retry and Abort for phase 0 failure', async () => {
    await triggerPhase0Error();
    expect(screen.queryByRole('button', { name: /Retry/i })).not.toBeNull();
    expect(screen.queryByRole('button', { name: /Abort/i })).not.toBeNull();
  });

  it('does NOT show the Step-5 "cannot be skipped" copy for phase 0 failure', async () => {
    await triggerPhase0Error();
    expect(screen.queryByText(/cannot be skipped/i)).toBeNull();
  });

  it('shows the error message from Rust (sign-in guidance) for phase 0 failure', async () => {
    await triggerPhase0Error();
    expect(screen.queryByText(/sign out and sign back in/i)).not.toBeNull();
  });
});

// ── 22.7.8: completion navigates to wizard ────────────────────────────────────

describe('22.7.8: completion', () => {
  it('shows account deleted confirmation message', async () => {
    setupAllSuccess();
    render(<AccountDeletionProgress {...defaultProps} />);
    await waitFor(() =>
      expect(screen.queryByText(/credentials and server data have been removed/i)).not.toBeNull()
    );
  });

  it('calls sign_out before onDeleted after all phases complete (B22)', async () => {
    const order: string[] = [];
    mockInvoke.mockImplementation((cmd: string, args?: { phase: number }) => {
      if (cmd === 'run_deletion_phase' && args !== undefined) {
        return Promise.resolve(phaseOk(args.phase, args.phase < 7 ? args.phase + 1 : null));
      }
      if (cmd === 'sign_out') {
        order.push('sign_out');
        return Promise.resolve();
      }
      return Promise.resolve();
    });
    const onDeleted = vi.fn(() => { order.push('onDeleted'); });
    render(<AccountDeletionProgress deleteWorkspaceDirs={false} onDeleted={onDeleted} onAbort={vi.fn()} />);
    await waitFor(() => expect(onDeleted).toHaveBeenCalled());
    expect(order).toEqual(['sign_out', 'onDeleted']);
  });
});
