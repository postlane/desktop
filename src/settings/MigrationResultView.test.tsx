// SPDX-License-Identifier: BUSL-1.1
// Tests for §22.5.8 — migration result and retry UI

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import MigrationResultView from './MigrationResultView';
import type { MigrationResult } from './MigrationBanner';

// ── Fixtures ──────────────────────────────────────────────────────────────────

function makeResult(overrides: Partial<MigrationResult['results'][0]>[] = []): MigrationResult {
  return {
    results: overrides.map((o, i) => ({
      repo_path: `/repos/repo-${i}`,
      repo_name: `repo-${i}`,
      status: { tag: 'success', posts_dir: `repo-${i}` },
      ...o,
    })),
  };
}

// ── 22.5.8: success display ───────────────────────────────────────────────────

describe('22.5.8: MigrationResultView — success', () => {
  it('shows success count when all repos migrated', () => {
    const result = makeResult([{}, {}]);
    render(<MigrationResultView result={result} workspacePath="/workspace" onRetry={vi.fn()} />);
    expect(screen.getByText(/2/)).toBeDefined();
    expect(screen.getByText(/migrat/i)).toBeDefined();
  });

  it('no Retry button when all repos succeeded', () => {
    const result = makeResult([{}]);
    render(<MigrationResultView result={result} workspacePath="/workspace" onRetry={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /retry/i })).toBeNull();
  });
});

// ── 22.5.8: failure display ───────────────────────────────────────────────────

describe('22.5.8: MigrationResultView — failure', () => {
  it('shows error code for failed repos', () => {
    const result = makeResult([
      { repo_name: 'bad-repo', status: { tag: 'verification_failed', error: 'PL-MIG-001: byte count mismatch' } },
    ]);
    render(<MigrationResultView result={result} workspacePath="/workspace" onRetry={vi.fn()} />);
    expect(screen.getByText(/PL-MIG-001/)).toBeDefined();
    expect(screen.getByText(/bad-repo/)).toBeDefined();
  });

  it('shows Retry button for repos with verification_failed status', () => {
    const result = makeResult([
      { status: { tag: 'verification_failed', error: 'PL-MIG-001' } },
    ]);
    render(<MigrationResultView result={result} workspacePath="/workspace" onRetry={vi.fn()} />);
    expect(screen.getByRole('button', { name: /retry/i })).toBeDefined();
  });

  it('Retry calls onRetry with only failed repo paths', () => {
    const onRetry = vi.fn();
    const result = makeResult([
      { repo_path: '/repos/good', status: { tag: 'success', posts_dir: 'good' } },
      { repo_path: '/repos/bad', status: { tag: 'verification_failed', error: 'PL-MIG-001' } },
    ]);
    render(<MigrationResultView result={result} workspacePath="/workspace" onRetry={onRetry} />);
    fireEvent.click(screen.getByRole('button', { name: /retry/i }));
    expect(onRetry).toHaveBeenCalledWith(['/repos/bad']);
  });

  it('project_id_mismatch shows warning, no Retry', () => {
    const result = makeResult([
      { repo_name: 'wrong-project', status: { tag: 'project_id_mismatch' } },
    ]);
    render(<MigrationResultView result={result} workspacePath="/workspace" onRetry={vi.fn()} />);
    expect(screen.getByText(/wrong-project/)).toBeDefined();
    // Retry not available for project_id mismatch
    expect(screen.queryByRole('button', { name: /retry/i })).toBeNull();
  });

  it('mixed: partial success shows both success count and Retry for failed', () => {
    const onRetry = vi.fn();
    const result = makeResult([
      { status: { tag: 'success', posts_dir: 'good' } },
      { status: { tag: 'verification_failed', error: 'PL-MIG-001' } },
    ]);
    render(<MigrationResultView result={result} workspacePath="/workspace" onRetry={onRetry} />);
    expect(screen.getByText(/PL-MIG-001/)).toBeDefined();
    expect(screen.getByRole('button', { name: /retry/i })).toBeDefined();
  });
});
