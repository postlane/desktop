// SPDX-License-Identifier: BUSL-1.1
// Tests for §22.5 migration flow component

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import MigrationFlow from './MigrationFlow';

const mockInvoke = vi.fn();
vi.mock('../ipc/invoke', () => ({ invoke: (...args: unknown[]) => mockInvoke(...args) }));

const WS_PATH = '/workspace/migrate';

function setupNoConflicts(migrationResult = {
  results: [{ repo_path: '/repo', repo_name: 'repo', status: { tag: 'success', posts_dir: 'repo' } }],
}) {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_workspace_path') return Promise.resolve(WS_PATH);
    if (cmd === 'get_migration_conflicts') return Promise.resolve([]);
    if (cmd === 'start_workspace_migration') return Promise.resolve(migrationResult);
    return Promise.resolve(null);
  });
}

function setupWithConflicts() {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_workspace_path') return Promise.resolve(WS_PATH);
    if (cmd === 'get_migration_conflicts') return Promise.resolve([{
      repo_path: '/repo', repo_name: 'my-repo',
      conflicts: [{
        field_key: 'style', label: 'Writing style',
        repo_value: 'Formal.', workspace_value: 'Direct.',
      }],
    }]);
    if (cmd === 'start_workspace_migration') return Promise.resolve({ results: [] });
    return Promise.resolve(null);
  });
}

// ── 22.5.16: no-conflict confirm path ────────────────────────────────────────

describe('MigrationFlow — no conflicts', () => {
  beforeEach(() => { vi.resetAllMocks(); setupNoConflicts(); });

  it('shows confirm screen after loading when no conflicts', async () => {
    render(<MigrationFlow projectId="proj-abc" onDone={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /confirm and migrate/i })).toBeDefined();
    });
  });

  it('calls start_workspace_migration with the correct workspace path on confirm', async () => {
    render(<MigrationFlow projectId="proj-abc" onDone={vi.fn()} />);
    const btn = await screen.findByRole('button', { name: /confirm and migrate/i });
    fireEvent.click(btn);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('start_workspace_migration', { workspacePath: WS_PATH });
    });
  });

  it('shows success result after migration completes', async () => {
    render(<MigrationFlow projectId="proj-abc" onDone={vi.fn()} />);
    const btn = await screen.findByRole('button', { name: /confirm and migrate/i });
    fireEvent.click(btn);
    await waitFor(() => {
      expect(screen.getByText(/migrated successfully/i)).toBeDefined();
    });
  });

  it('calls onDone when Cancel is clicked on confirm screen', async () => {
    const onDone = vi.fn();
    render(<MigrationFlow projectId="proj-abc" onDone={onDone} />);
    const btn = await screen.findByRole('button', { name: /cancel/i });
    fireEvent.click(btn);
    expect(onDone).toHaveBeenCalledOnce();
  });
});

// ── 22.5.16: conflicts path ───────────────────────────────────────────────────

describe('MigrationFlow — conflicts', () => {
  beforeEach(() => { vi.resetAllMocks(); setupWithConflicts(); });

  it('shows ConflictDiffView with repo name and field label when conflicts exist', async () => {
    render(<MigrationFlow projectId="proj-abc" onDone={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText(/my-repo/i)).toBeDefined();
      expect(screen.getByText(/Writing style/i)).toBeDefined();
    });
  });

  it('calls start_workspace_migration after confirming conflict', async () => {
    render(<MigrationFlow projectId="proj-abc" onDone={vi.fn()} />);
    const btn = await screen.findByRole('button', { name: /confirm and migrate/i });
    fireEvent.click(btn);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('start_workspace_migration', { workspacePath: WS_PATH });
    });
  });
});

// ── no workspace registered ───────────────────────────────────────────────────

describe('MigrationFlow — no workspace', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_workspace_path') return Promise.resolve(null);
      return Promise.resolve(null);
    });
  });

  it('shows error message when no workspace found for project', async () => {
    render(<MigrationFlow projectId="proj-abc" onDone={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText(/no workspace/i)).toBeDefined();
    });
  });
});

// ── UX-C6: migration errors show real message, not "No workspace" ────────────

describe('MigrationFlow — migration error', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_workspace_path') return Promise.resolve(WS_PATH);
      if (cmd === 'get_migration_conflicts') return Promise.resolve([]);
      if (cmd === 'start_workspace_migration') return Promise.reject(new Error('PL-MIG-002: disk full'));
      return Promise.resolve(null);
    });
  });

  it('shows actual error message when migration fails', async () => {
    render(<MigrationFlow projectId="proj-abc" onDone={vi.fn()} />);
    const btn = await screen.findByRole('button', { name: /confirm and migrate/i });
    fireEvent.click(btn);
    await waitFor(() => {
      expect(screen.queryByText(/no workspace/i)).toBeNull();
      expect(screen.getByRole('alert')).toBeDefined();
    });
  });
});

describe('MigrationFlow — retry error', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_workspace_path') return Promise.resolve(WS_PATH);
      if (cmd === 'get_migration_conflicts') return Promise.resolve([]);
      if (cmd === 'start_workspace_migration') return Promise.resolve({
        results: [{ repo_path: '/repo', repo_name: 'repo', status: { tag: 'verification_failed', error: 'PL-MIG-001' } }],
      });
      if (cmd === 'retry_workspace_migration') return Promise.reject(new Error('PL-MIG-003: permission denied'));
      return Promise.resolve(null);
    });
  });

  it('shows actual error message when retry fails', async () => {
    render(<MigrationFlow projectId="proj-abc" onDone={vi.fn()} />);
    const confirmBtn = await screen.findByRole('button', { name: /confirm and migrate/i });
    fireEvent.click(confirmBtn);
    const retryBtn = await screen.findByRole('button', { name: /retry/i });
    fireEvent.click(retryBtn);
    await waitFor(() => {
      expect(screen.queryByText(/no workspace/i)).toBeNull();
      expect(screen.getByRole('alert')).toBeDefined();
    });
  });
});

// ── retry ─────────────────────────────────────────────────────────────────────

describe('MigrationFlow — retry', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_workspace_path') return Promise.resolve(WS_PATH);
      if (cmd === 'get_migration_conflicts') return Promise.resolve([]);
      if (cmd === 'start_workspace_migration') return Promise.resolve({
        results: [{ repo_path: '/repo', repo_name: 'repo', status: { tag: 'verification_failed', error: 'PL-MIG-001' } }],
      });
      if (cmd === 'retry_workspace_migration') return Promise.resolve({
        results: [{ repo_path: '/repo', repo_name: 'repo', status: { tag: 'success', posts_dir: 'repo' } }],
      });
      return Promise.resolve(null);
    });
  });

  it('calls retry_workspace_migration with failed repo paths', async () => {
    render(<MigrationFlow projectId="proj-abc" onDone={vi.fn()} />);
    const confirmBtn = await screen.findByRole('button', { name: /confirm and migrate/i });
    fireEvent.click(confirmBtn);
    const retryBtn = await screen.findByRole('button', { name: /retry/i });
    fireEvent.click(retryBtn);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('retry_workspace_migration', {
        workspacePath: WS_PATH,
        repoPaths: ['/repo'],
      });
    });
  });
});
