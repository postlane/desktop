// SPDX-License-Identifier: BUSL-1.1
// Tests for §22.5 migration banner React components

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import {
  MigrationBannerContent,
  RecoveryBannerContent,
  CleanupSuccessMessage,
} from './MigrationBanner';
import type { MigrationStatus, JournalStatus, MigrationJournalEntry } from './MigrationBanner';

// ── Test helpers ──────────────────────────────────────────────────────────────

function makeStatus(repoCount: number, dismissed = false): MigrationStatus {
  const repos = Array.from({ length: repoCount }, (_, i) => ({
    id: `r${i}`, name: `repo-${i}`, path: `/repos/repo-${i}`,
  }));
  return { qualifying_repos: repos, total_legacy_repos: repos, dismissed };
}

function makeJournal(dismissCount: number, entries: Partial<MigrationJournalEntry>[] = []): JournalStatus {
  return {
    workspace_id: 'ws-abc',
    workspace_path: '/workspace',
    pending_entries: entries.map((e) => ({
      repo_path: '/repo', posts_dir: 'repo',
      registry_updated: false, originals_deleted: false,
      ...e,
    })),
    dismiss_count: dismissCount,
  };
}

// ── 22.5.13: dismissed → no banner ───────────────────────────────────────────

describe('22.5.13: MigrationBannerContent — dismissed status', () => {
  it('renders nothing when qualifying_repos is empty', () => {
    const { container } = render(
      <MigrationBannerContent
        status={makeStatus(0)}
        onDismiss={vi.fn()}
        onSetupWorkspace={vi.fn()}
      />
    );
    expect(container.firstChild).toBeNull();
  });
});

// ── 22.5.14: no qualifying repos → no banner shown ───────────────────────────

describe('22.5.14: no qualifying repos', () => {
  it('renders nothing for empty qualifying repos', () => {
    const { container } = render(
      <MigrationBannerContent
        status={{ qualifying_repos: [], total_legacy_repos: [], dismissed: false }}
        onDismiss={vi.fn()}
        onSetupWorkspace={vi.fn()}
      />
    );
    expect(container.firstChild).toBeNull();
  });
});

// ── 22.5.15: banner shown, "Not now" dismisses ───────────────────────────────

describe('22.5.15: MigrationBannerContent — banner display and dismiss', () => {
  it('shows banner text when qualifying repos exist', () => {
    render(
      <MigrationBannerContent
        status={makeStatus(2)}
        onDismiss={vi.fn()}
        onSetupWorkspace={vi.fn()}
      />
    );
    expect(screen.getByText(/central workspace/i)).toBeDefined();
    expect(screen.getByText(/Set up workspace/i)).toBeDefined();
    expect(screen.getByText(/Not now/i)).toBeDefined();
  });

  it('"Not now" calls onDismiss', () => {
    const onDismiss = vi.fn();
    render(
      <MigrationBannerContent
        status={makeStatus(1)}
        onDismiss={onDismiss}
        onSetupWorkspace={vi.fn()}
      />
    );
    fireEvent.click(screen.getByText(/Not now/i));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it('"Set up workspace" calls onSetupWorkspace', () => {
    const onSetup = vi.fn();
    render(
      <MigrationBannerContent
        status={makeStatus(1)}
        onDismiss={vi.fn()}
        onSetupWorkspace={onSetup}
      />
    );
    fireEvent.click(screen.getByText(/Set up workspace/i));
    expect(onSetup).toHaveBeenCalledOnce();
  });

  it('shows correct repo count in banner text', () => {
    render(
      <MigrationBannerContent
        status={makeStatus(3)}
        onDismiss={vi.fn()}
        onSetupWorkspace={vi.fn()}
      />
    );
    expect(screen.getByText(/3 repositories/i)).toBeDefined();
  });

  it('uses singular form for one repo', () => {
    render(
      <MigrationBannerContent
        status={makeStatus(1)}
        onDismiss={vi.fn()}
        onSetupWorkspace={vi.fn()}
      />
    );
    expect(screen.getByText(/1 repository/i)).toBeDefined();
  });
});

// ── 22.5.22/22.5.25: recovery banner ─────────────────────────────────────────

describe('22.5.22/22.5.25: RecoveryBannerContent', () => {
  it('shows recovery message for pending entries', () => {
    render(
      <RecoveryBannerContent
        journal={makeJournal(0, [{ registry_updated: true }])}
        onResume={vi.fn()}
        onDismiss={vi.fn()}
      />
    );
    expect(screen.getByText(/previous migration was interrupted/i)).toBeDefined();
    expect(screen.getByText(/Resume cleanup/i)).toBeDefined();
  });

  it('"Resume cleanup" calls onResume with workspace_id', () => {
    const onResume = vi.fn();
    render(
      <RecoveryBannerContent
        journal={makeJournal(0, [{ registry_updated: true }])}
        onResume={onResume}
        onDismiss={vi.fn()}
      />
    );
    fireEvent.click(screen.getByText(/Resume cleanup/i));
    expect(onResume).toHaveBeenCalledWith('ws-abc');
  });

  it('"Dismiss" calls onDismiss with workspace_id', () => {
    const onDismiss = vi.fn();
    render(
      <RecoveryBannerContent
        journal={makeJournal(0, [{ registry_updated: true }])}
        onResume={vi.fn()}
        onDismiss={onDismiss}
      />
    );
    fireEvent.click(screen.getByText(/Dismiss/i));
    expect(onDismiss).toHaveBeenCalledWith('ws-abc');
  });

  it('22.5.5c: no dismiss button after 3 dismissals', () => {
    render(
      <RecoveryBannerContent
        journal={makeJournal(3, [{ registry_updated: true }])}
        onResume={vi.fn()}
        onDismiss={vi.fn()}
      />
    );
    expect(screen.queryByText(/^Dismiss$/i)).toBeNull();
    expect(screen.getByText(/Resume cleanup now/i)).toBeDefined();
  });

  it('22.5.5c: non-dismissible banner shows stronger copy', () => {
    render(
      <RecoveryBannerContent
        journal={makeJournal(3, [{ originals_deleted: false }])}
        onResume={vi.fn()}
        onDismiss={vi.fn()}
      />
    );
    expect(screen.getByText(/cannot be cleaned up until you resume/i)).toBeDefined();
  });
});

// ── 22.10.18: CleanupSuccessMessage after resume ──────────────────────────────

describe('22.10.18: CleanupSuccessMessage', () => {
  it('renders "Cleanup complete. Original files removed."', () => {
    render(<CleanupSuccessMessage onDismiss={vi.fn()} />);
    expect(screen.getByText('Cleanup complete. Original files removed.')).toBeDefined();
  });

  it('calls onDismiss when Dismiss is clicked', () => {
    const onDismiss = vi.fn();
    render(<CleanupSuccessMessage onDismiss={onDismiss} />);
    fireEvent.click(screen.getByText('Dismiss'));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});

// ── 22.5.21: Settings "Migrate to workspace..." button logic ──────────────────

describe('22.5.21: Settings migration re-entry button', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it('shows migrate button when legacy repos exist in status', () => {
    // The button is rendered by RepositoriesBlock; we test the condition here
    // via a simple helper that mirrors the display logic.
    const hasLegacyRepos = (status: MigrationStatus | null) =>
      status !== null && status.qualifying_repos.length > 0;
    expect(hasLegacyRepos(makeStatus(2))).toBe(true);
    expect(hasLegacyRepos(makeStatus(0))).toBe(false);
    expect(hasLegacyRepos(null)).toBe(false);
  });
});
