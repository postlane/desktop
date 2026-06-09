// SPDX-License-Identifier: BUSL-1.1
// Tests for §22.5.9–22.5.12 — Settings migration re-entry

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import type { MigrationStatus, LegacyRepoInfo } from './MigrationBanner';

// ── Mock setup ────────────────────────────────────────────────────────────────

const mockInvoke = vi.fn();
vi.mock('../ipc/invoke', () => ({ invoke: (...args: unknown[]) => mockInvoke(...args) }));

function makeLegacyRepo(n = 1): LegacyRepoInfo[] {
  return Array.from({ length: n }, (_, i) => ({
    id: `r${i}`, name: `repo-${i}`, path: `/repos/repo-${i}`,
  }));
}

function makeStatus(total: number, qualifying: number, dismissed = false): MigrationStatus {
  return {
    qualifying_repos: makeLegacyRepo(qualifying),
    total_legacy_repos: makeLegacyRepo(total),
    dismissed,
  };
}

// Each test imports RepositoriesBlock freshly to pick up mock.
// We use a factory function to keep tests independent.
async function renderBlock(
  status: MigrationStatus,
  projectId = 'proj-abc',
) {
  const { vi: viTest } = await import('vitest');
  viTest.resetModules();

  viTest.doMock('./MigrationBanner', () => ({
    useMigrationStatus: () => ({ status, dismiss: viTest.fn() }),
    useJournalStatuses: () => ({ statuses: [], resume: viTest.fn(), dismissSession: viTest.fn() }),
  }));

  const { default: RepositoriesBlock } = await import('./RepositoriesBlock');
  return render(<RepositoriesBlock projectId={projectId} isOwner />);
}

// ── 22.5.9: button visible when legacy repos exist ────────────────────────────

describe('22.5.9: "Migrate to workspace..." button', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_repo_connection_status') return Promise.resolve([]);
      return Promise.resolve(null);
    });
  });

  it('shows button when total_legacy_repos is non-empty', async () => {
    const status = makeStatus(2, 2);
    await renderBlock(status);
    await waitFor(() => {
      expect(screen.queryByText(/migrate to workspace/i)).not.toBeNull();
    });
  });

  it('hides button when total_legacy_repos is empty (22.5.11)', async () => {
    const status = makeStatus(0, 0);
    await renderBlock(status);
    await waitFor(() => {
      expect(screen.queryByText(/migrate to workspace/i)).toBeNull();
    });
  });

  it('shows button even when dismissed (22.5.9: ignores dismissed flag)', async () => {
    const status = makeStatus(1, 0, true);  // dismissed, but has legacy repos
    await renderBlock(status);
    await waitFor(() => {
      expect(screen.queryByText(/migrate to workspace/i)).not.toBeNull();
    });
  });
});

// ── 22.10.9: clicking button opens MigrationFlow ────────────────────────────

describe('22.10.9: clicking "Migrate to workspace..." opens migration flow', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_repo_connection_status') return Promise.resolve([]);
      return Promise.resolve(null);
    });
  });

  it('renders MigrationFlow after button click', async () => {
    // Stub MigrationFlow so we avoid real IPC calls inside it.
    vi.doMock('./MigrationFlow', () => ({
      default: () => <div data-testid="migration-flow-sentinel" />,
    }));

    const status = makeStatus(1, 1);
    await renderBlock(status);
    const btn = await screen.findByText(/migrate to workspace/i);
    fireEvent.click(btn);
    await waitFor(() => {
      expect(screen.queryByTestId('migration-flow-sentinel')).not.toBeNull();
    });
  });
});

// ── 22.5.12: telemetry on button click ───────────────────────────────────────

describe('22.5.12: note_migration_reentered telemetry', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_repo_connection_status') return Promise.resolve([]);
      return Promise.resolve(null);
    });
  });

  it('calls note_migration_reentered when button is clicked', async () => {
    const status = makeStatus(1, 0);
    await renderBlock(status);
    const btn = await screen.findByText(/migrate to workspace/i);
    fireEvent.click(btn);
    expect(mockInvoke).toHaveBeenCalledWith('note_migration_reentered');
  });
});
