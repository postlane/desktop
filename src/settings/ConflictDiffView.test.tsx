// SPDX-License-Identifier: BUSL-1.1
// Tests for §22.5.6 and §22.5.19 — conflict diff view

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import ConflictDiffView from './ConflictDiffView';
import type { FieldConflict } from '../settings/MigrationBanner';

// ── Fixtures ──────────────────────────────────────────────────────────────────

const CONFLICTS: FieldConflict[] = [
  { field_key: 'llm.provider', label: 'AI provider', repo_value: 'anthropic', workspace_value: 'openai' },
  { field_key: 'style', label: 'Writing style', repo_value: 'Direct.', workspace_value: 'Formal.' },
];

// ── 22.5.6: two-column diff table ─────────────────────────────────────────────

describe('22.5.6: ConflictDiffView', () => {
  it('renders human-readable labels, not raw JSON keys', () => {
    render(
      <ConflictDiffView
        repoName="my-repo"
        conflicts={CONFLICTS}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    expect(screen.getByText('AI provider')).toBeDefined();
    expect(screen.getByText('Writing style')).toBeDefined();
    // Raw keys must NOT appear
    expect(screen.queryByText('llm.provider')).toBeNull();
    expect(screen.queryByText('style')).toBeNull();
  });

  it('shows repo value and workspace value for each conflict', () => {
    render(
      <ConflictDiffView
        repoName="my-repo"
        conflicts={CONFLICTS}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    expect(screen.getByText('anthropic')).toBeDefined();
    expect(screen.getByText('openai')).toBeDefined();
    expect(screen.getByText('Direct.')).toBeDefined();
    expect(screen.getByText('Formal.')).toBeDefined();
  });

  it('calls onConfirm when Confirm is clicked', () => {
    const onConfirm = vi.fn();
    render(
      <ConflictDiffView repoName="my-repo" conflicts={CONFLICTS} onConfirm={onConfirm} onCancel={vi.fn()} />
    );
    fireEvent.click(screen.getByRole('button', { name: /confirm/i }));
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it('calls onCancel when Cancel is clicked', () => {
    const onCancel = vi.fn();
    render(
      <ConflictDiffView repoName="my-repo" conflicts={CONFLICTS} onConfirm={vi.fn()} onCancel={onCancel} />
    );
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('shows repo name in heading', () => {
    render(
      <ConflictDiffView repoName="special-repo" conflicts={CONFLICTS} onConfirm={vi.fn()} onCancel={vi.fn()} />
    );
    expect(screen.getByText(/special-repo/)).toBeDefined();
  });

  it('indicates workspace wins on conflict (column header)', () => {
    render(
      <ConflictDiffView repoName="my-repo" conflicts={CONFLICTS} onConfirm={vi.fn()} onCancel={vi.fn()} />
    );
    // Column header must mention "workspace" and "wins"
    expect(screen.getByText(/workspace.*wins/i)).toBeDefined();
  });
});

// ── 22.5.19: Cancel aborts migration (no command called) ─────────────────────

describe('22.5.19: Cancel in ConflictDiffView — no migration called', () => {
  it('onCancel does not trigger onConfirm', () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(
      <ConflictDiffView repoName="my-repo" conflicts={CONFLICTS} onConfirm={onConfirm} onCancel={onCancel} />
    );
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onConfirm).not.toHaveBeenCalled();
    expect(onCancel).toHaveBeenCalledOnce();
  });
});
