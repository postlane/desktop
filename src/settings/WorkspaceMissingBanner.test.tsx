// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '../ipc/invoke';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import WorkspaceMissingBanner, { type WorkspaceCheckResult } from './WorkspaceMissingBanner';

const mockInvoke = vi.mocked(invoke);
const mockOpen = vi.mocked(openDialog);

function makeRenamedResult(candidates: Array<{ name: string; path: string; modified_secs: number }>): WorkspaceCheckResult {
  return {
    workspace_id: 'proj-1',
    workspace_path: '/Users/hugo/code/myorg',
    workspace_name: 'myorg',
    status: { tag: 'renamed', candidates },
  };
}

function makeMissingResult(): WorkspaceCheckResult {
  return {
    workspace_id: 'proj-1',
    workspace_path: '/Users/hugo/code/myorg',
    workspace_name: 'myorg',
    status: { tag: 'missing' },
  };
}

beforeEach(() => {
  mockInvoke.mockReset();
  mockOpen.mockReset();
});

// ── 22.3.27: single rename candidate ─────────────────────────────────────────

describe('renamed workspace — single candidate', () => {
  const singleCandidate = [{ name: 'myorg-renamed', path: '/Users/hugo/code/myorg-renamed', modified_secs: 1000 }];

  it('shows old name and new candidate name', () => {
    render(<WorkspaceMissingBanner
      result={makeRenamedResult(singleCandidate)}
      onResolved={vi.fn()} onDismiss={vi.fn()}
    />);
    expect(screen.getByText(/myorg/)).toBeInTheDocument();
    expect(screen.getByText(/myorg-renamed/)).toBeInTheDocument();
  });

  it('shows Update path and Locate manually buttons', () => {
    render(<WorkspaceMissingBanner
      result={makeRenamedResult(singleCandidate)}
      onResolved={vi.fn()} onDismiss={vi.fn()}
    />);
    expect(screen.getByRole('button', { name: /update path/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /locate manually/i })).toBeInTheDocument();
  });

  it('calls update_workspace_path and onResolved when Update path clicked', async () => {
    mockInvoke.mockResolvedValue(null);
    const onResolved = vi.fn();
    render(<WorkspaceMissingBanner
      result={makeRenamedResult(singleCandidate)}
      onResolved={onResolved} onDismiss={vi.fn()}
    />);
    fireEvent.click(screen.getByRole('button', { name: /update path/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('update_workspace_path', {
      workspaceId: 'proj-1',
      newPath: '/Users/hugo/code/myorg-renamed',
    }));
    expect(onResolved).toHaveBeenCalled();
  });

  it('does NOT call update_workspace_path before button is clicked', () => {
    render(<WorkspaceMissingBanner
      result={makeRenamedResult(singleCandidate)}
      onResolved={vi.fn()} onDismiss={vi.fn()}
    />);
    expect(mockInvoke).not.toHaveBeenCalledWith('update_workspace_path', expect.anything());
  });
});

// ── 22.3.28: multiple rename candidates ──────────────────────────────────────

describe('renamed workspace — multiple candidates', () => {
  const twoCandidates = [
    { name: 'myorg-a', path: '/Users/hugo/code/myorg-a', modified_secs: 500 },
    { name: 'myorg-b', path: '/Users/hugo/code/myorg-b', modified_secs: 2000 },
  ];

  it('shows both candidate names', () => {
    render(<WorkspaceMissingBanner
      result={makeRenamedResult(twoCandidates)}
      onResolved={vi.fn()} onDismiss={vi.fn()}
    />);
    expect(screen.getByText(/myorg-a/)).toBeInTheDocument();
    expect(screen.getByText(/myorg-b/)).toBeInTheDocument();
  });

  it('pre-selects the most recently modified candidate (highest modified_secs)', () => {
    render(<WorkspaceMissingBanner
      result={makeRenamedResult(twoCandidates)}
      onResolved={vi.fn()} onDismiss={vi.fn()}
    />);
    // myorg-b has higher modified_secs (2000 > 500) — should be the pre-selected option
    const select = screen.getByRole('combobox');
    expect((select as HTMLSelectElement).value).toBe('/Users/hugo/code/myorg-b');
  });

  it('calls update_workspace_path with the selected candidate path', async () => {
    mockInvoke.mockResolvedValue(null);
    const onResolved = vi.fn();
    render(<WorkspaceMissingBanner
      result={makeRenamedResult(twoCandidates)}
      onResolved={onResolved} onDismiss={vi.fn()}
    />);
    // Change selection to myorg-a
    const select = screen.getByRole('combobox');
    fireEvent.change(select, { target: { value: '/Users/hugo/code/myorg-a' } });
    fireEvent.click(screen.getByRole('button', { name: /update path/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('update_workspace_path', {
      workspaceId: 'proj-1',
      newPath: '/Users/hugo/code/myorg-a',
    }));
    expect(onResolved).toHaveBeenCalled();
  });
});

// ── 22.3.29: missing workspace ────────────────────────────────────────────────

describe('missing workspace', () => {
  it('shows workspace not found message with old path', () => {
    render(<WorkspaceMissingBanner
      result={makeMissingResult()}
      onResolved={vi.fn()} onDismiss={vi.fn()}
    />);
    expect(screen.getByText(/workspace folder not found/i)).toBeInTheDocument();
    expect(screen.getByText(/\/Users\/hugo\/code\/myorg/)).toBeInTheDocument();
  });

  it('shows Locate folder and Dismiss buttons', () => {
    render(<WorkspaceMissingBanner
      result={makeMissingResult()}
      onResolved={vi.fn()} onDismiss={vi.fn()}
    />);
    expect(screen.getByRole('button', { name: /locate folder/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /dismiss/i })).toBeInTheDocument();
  });

  it('calls onDismiss when Dismiss clicked', () => {
    const onDismiss = vi.fn();
    render(<WorkspaceMissingBanner
      result={makeMissingResult()}
      onResolved={vi.fn()} onDismiss={onDismiss}
    />);
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(onDismiss).toHaveBeenCalled();
  });

  it('opens folder picker and calls locate_workspace_folder on Locate folder click', async () => {
    mockOpen.mockResolvedValue('/Users/hugo/code/new-location');
    mockInvoke.mockResolvedValue(null);
    const onResolved = vi.fn();
    render(<WorkspaceMissingBanner
      result={makeMissingResult()}
      onResolved={onResolved} onDismiss={vi.fn()}
    />);
    fireEvent.click(screen.getByRole('button', { name: /locate folder/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('locate_workspace_folder', {
      workspaceId: 'proj-1',
      folderPath: '/Users/hugo/code/new-location',
    }));
    expect(onResolved).toHaveBeenCalled();
  });
});

// ── 22.3.30: PL-WS-002 error on locate ───────────────────────────────────────

describe('locate folder — project_id mismatch', () => {
  it('shows PL-WS-002 error and does not call onResolved', async () => {
    mockOpen.mockResolvedValue('/Users/hugo/code/wrong-project');
    mockInvoke.mockRejectedValue('PL-WS-002: This folder belongs to a different Postlane project');
    const onResolved = vi.fn();
    render(<WorkspaceMissingBanner
      result={makeMissingResult()}
      onResolved={onResolved} onDismiss={vi.fn()}
    />);
    fireEvent.click(screen.getByRole('button', { name: /locate folder/i }));
    await waitFor(() => expect(screen.getByText(/PL-WS-002/)).toBeInTheDocument());
    expect(onResolved).not.toHaveBeenCalled();
  });

  it('does not call update_workspace_path on PL-WS-002', async () => {
    mockOpen.mockResolvedValue('/Users/hugo/code/wrong-project');
    mockInvoke.mockRejectedValue('PL-WS-002: This folder belongs to a different Postlane project');
    render(<WorkspaceMissingBanner
      result={makeMissingResult()}
      onResolved={vi.fn()} onDismiss={vi.fn()}
    />);
    fireEvent.click(screen.getByRole('button', { name: /locate folder/i }));
    await waitFor(() => expect(mockInvoke).not.toHaveBeenCalledWith('update_workspace_path', expect.anything()));
  });

  it('does not show error when locate folder succeeds', async () => {
    mockOpen.mockResolvedValue('/Users/hugo/code/correct-project');
    mockInvoke.mockResolvedValue(null);
    render(<WorkspaceMissingBanner
      result={makeMissingResult()}
      onResolved={vi.fn()} onDismiss={vi.fn()}
    />);
    fireEvent.click(screen.getByRole('button', { name: /locate folder/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('locate_workspace_folder', expect.anything()));
    expect(screen.queryByText(/PL-WS-002/)).not.toBeInTheDocument();
  });
});

// ── Locate manually (from renamed banner) ────────────────────────────────────

describe('locate manually button in renamed banner', () => {
  const singleCandidate = [{ name: 'myorg-renamed', path: '/Users/hugo/code/myorg-renamed', modified_secs: 1000 }];

  it('opens folder picker and calls locate_workspace_folder', async () => {
    mockOpen.mockResolvedValue('/Users/hugo/code/manual-pick');
    mockInvoke.mockResolvedValue(null);
    const onResolved = vi.fn();
    render(<WorkspaceMissingBanner
      result={makeRenamedResult(singleCandidate)}
      onResolved={onResolved} onDismiss={vi.fn()}
    />);
    fireEvent.click(screen.getByRole('button', { name: /locate manually/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('locate_workspace_folder', {
      workspaceId: 'proj-1',
      folderPath: '/Users/hugo/code/manual-pick',
    }));
    expect(onResolved).toHaveBeenCalled();
  });
});
