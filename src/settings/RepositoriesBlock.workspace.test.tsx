// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('../wizard/AddRepoModal', () => ({
  default: ({ onClose }: { onClose: () => void }) => (
    <div data-testid="add-repo-modal"><button onClick={onClose}>Close modal</button></div>
  ),
}))
vi.mock('./WorkspaceConfirmModal', () => ({
  default: ({ result, onConfirm, onCancel }: {
    result: { workspace_id: string; discovered_repos: Array<{ path: string }> };
    onConfirm: (paths: string[]) => void;
    onCancel: () => void;
  }) => (
    <div data-testid="workspace-confirm-modal">
      <button onClick={() => onConfirm(result.discovered_repos.map((r) => r.path))}>Confirm all</button>
      <button onClick={onCancel}>Cancel workspace</button>
    </div>
  ),
}))
vi.mock('./VoiceGuideHint', () => ({
  default: ({ workspacePath, onDismiss }: { workspacePath: string; onDismiss: () => void }) => (
    <div data-testid="voice-guide-hint" data-workspace-path={workspacePath}>
      <button onClick={onDismiss}>Dismiss hint</button>
    </div>
  ),
}))

import { invoke } from '../ipc/invoke'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import RepositoriesBlock from './RepositoriesBlock'

const mockInvoke = vi.mocked(invoke)
const mockOpen = vi.mocked(openDialog)

// ── Helpers ────────────────────────────────────────────────────────────────────

interface RepoConnectionStatus {
  repo_id: string | null; github_full_name: string | null; local_path: string | null;
  display_name: string; github_app_connected: boolean; folder_registered: boolean;
  cli_initialized: boolean; project_id_mismatch: boolean;
}

function makeRow(overrides: Partial<RepoConnectionStatus> = {}): RepoConnectionStatus {
  return {
    repo_id: 'repo-1', github_full_name: null, local_path: '/repos/myrepo',
    display_name: 'MyRepo', github_app_connected: false, folder_registered: true,
    cli_initialized: true, project_id_mismatch: false,
    ...overrides,
  }
}

const singleResult = {
  workspace_id: 'proj-1',
  workspace_path: '/Users/hugo/code/myorg',
  discovered_repos: [{ name: 'frontend', path: '/Users/hugo/code/myorg/frontend', posts_dir: 'frontend' }],
}

const multiResult = {
  workspace_id: 'proj-1',
  workspace_path: '/Users/hugo/code/myorg',
  discovered_repos: [
    { name: 'frontend', path: '/Users/hugo/code/myorg/frontend', posts_dir: 'frontend' },
    { name: 'backend', path: '/Users/hugo/code/myorg/backend', posts_dir: 'backend' },
  ],
}

function setupSingleRepoMock() {
  mockOpen.mockResolvedValue('/Users/hugo/code/myorg')
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'get_repo_connection_status') return [makeRow()]
    if (cmd === 'add_workspace') return singleResult
    if (cmd === 'confirm_workspace_repos') return null
    return null
  })
}

function setupMultiRepoMock() {
  mockOpen.mockResolvedValue('/Users/hugo/code/myorg')
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'get_repo_connection_status') return [makeRow()]
    if (cmd === 'add_workspace') return multiResult
    if (cmd === 'confirm_workspace_repos') return null
    return null
  })
}

beforeEach(() => { vi.clearAllMocks() })

// ── 22.3.1 — Add workspace CTA ────────────────────────────────────────────────

describe('RepositoriesBlock — Add workspace CTA (22.3.1)', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      return null
    })
  })

  it('shows "Add workspace" as primary CTA for owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /^Add workspace$/i })).toBeInTheDocument()
    )
  })

  it('shows "Add individual repository" option for owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Add individual repository/i })).toBeInTheDocument()
    )
  })

  it('does not show a button labeled "Add repository" alone', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: /^Add repository$/i })).not.toBeInTheDocument()
    )
  })
})

// ── 22.3.7 — Folder picker wired to add_workspace ────────────────────────────

describe('RepositoriesBlock — Add workspace folder picker (22.3.7)', () => {
  it('calls dialog open with directory:true when "Add workspace" is clicked', async () => {
    mockOpen.mockResolvedValue(null)
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() => expect(mockOpen).toHaveBeenCalledWith({ directory: true }))
  })

  it('calls add_workspace with folderPath and projectId after selection', async () => {
    mockOpen.mockResolvedValue('/Users/hugo/code/myorg')
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'add_workspace') return singleResult
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('add_workspace', {
        folderPath: '/Users/hugo/code/myorg',
        projectId: 'proj-1',
      })
    )
  })

  it('does nothing when the user cancels the folder picker', async () => {
    mockOpen.mockResolvedValue(null)
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() => expect(mockOpen).toHaveBeenCalled())
    expect(mockInvoke).not.toHaveBeenCalledWith('add_workspace', expect.anything())
  })
})

// ── 22.3.5 — Single-repo auto-confirm ─────────────────────────────────────────

describe('RepositoriesBlock — single-repo auto-confirm (22.3.5)', () => {
  it('calls confirm_workspace_repos automatically for single-repo workspace', async () => {
    setupSingleRepoMock()
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('confirm_workspace_repos', {
        workspaceId: 'proj-1',
        selectedPaths: ['/Users/hugo/code/myorg/frontend'],
      })
    )
  })

  it('does not show WorkspaceConfirmModal for single-repo workspace', async () => {
    setupSingleRepoMock()
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('confirm_workspace_repos', expect.anything())
    )
    expect(screen.queryByTestId('workspace-confirm-modal')).not.toBeInTheDocument()
  })

  it('refreshes the repo list after single-repo auto-confirm', async () => {
    setupSingleRepoMock()
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter((c) => c[0] === 'get_repo_connection_status')
      expect(calls.length).toBeGreaterThan(1)
    })
  })
})

// ── 22.3.11 — Single-repo informational toast ─────────────────────────────────

describe('RepositoriesBlock — single-repo toast (22.3.11)', () => {
  it('shows informational toast text before auto-registering', async () => {
    mockOpen.mockResolvedValue('/Users/hugo/code/myorg')
    let resolveConfirm: (v: null) => void = () => {};
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'add_workspace') return singleResult
      if (cmd === 'confirm_workspace_repos') return new Promise<null>((res) => { resolveConfirm = res; })
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() =>
      expect(screen.getByText(/Creating workspace at/i)).toBeInTheDocument()
    )
    expect(screen.getByText(/Postlane files will be added to frontend\//i)).toBeInTheDocument()
    resolveConfirm(null)
  })
})

// ── 22.3.5a / 22.3.5c — Voice guide hint after creation ──────────────────────

describe('RepositoriesBlock — voice guide hint (22.3.5a, 22.3.5c)', () => {
  it('shows voice guide hint after single-repo workspace creation', async () => {
    setupSingleRepoMock()
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() => expect(screen.getByTestId('voice-guide-hint')).toBeInTheDocument())
    expect(screen.getByTestId('voice-guide-hint'))
      .toHaveAttribute('data-workspace-path', '/Users/hugo/code/myorg')
  })

  it('dismisses voice guide hint on close', async () => {
    setupSingleRepoMock()
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() => expect(screen.getByTestId('voice-guide-hint')).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Dismiss hint/i }))
    expect(screen.queryByTestId('voice-guide-hint')).not.toBeInTheDocument()
  })

  it('shows voice guide hint after multi-repo workspace confirmation', async () => {
    setupMultiRepoMock()
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() => expect(screen.getByTestId('workspace-confirm-modal')).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /^Confirm all$/i }))
    await waitFor(() => expect(screen.getByTestId('voice-guide-hint')).toBeInTheDocument())
  })
})

// ── 22.3.3 — Multi-repo confirmation modal ────────────────────────────────────

describe('RepositoriesBlock — multi-repo confirmation modal (22.3.3)', () => {
  it('shows WorkspaceConfirmModal when multiple repos are discovered', async () => {
    setupMultiRepoMock()
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() =>
      expect(screen.getByTestId('workspace-confirm-modal')).toBeInTheDocument()
    )
  })

  it('does not auto-call confirm_workspace_repos before user confirms multi-repo', async () => {
    setupMultiRepoMock()
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() => expect(screen.getByTestId('workspace-confirm-modal')).toBeInTheDocument())
    expect(mockInvoke).not.toHaveBeenCalledWith('confirm_workspace_repos', expect.anything())
  })

  it('hides WorkspaceConfirmModal after user cancels', async () => {
    setupMultiRepoMock()
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    await waitFor(() => expect(screen.getByTestId('workspace-confirm-modal')).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Cancel workspace/i }))
    expect(screen.queryByTestId('workspace-confirm-modal')).not.toBeInTheDocument()
  })
})

// ── 22.3.17 / 22.3.20 — Rescan workspace button ───────────────────────────────

describe('RepositoriesBlock — Rescan workspace (22.3.17, 22.3.20)', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      return null
    })
  })

  it('shows "Rescan workspace" button for owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('button', { name: /Rescan workspace/i })).toBeInTheDocument())
  })

  it('hides "Rescan workspace" button for non-owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={false} />)
    await waitFor(() => expect(screen.getByText('MyRepo')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Rescan workspace/i })).not.toBeInTheDocument()
  })

  it('calls rescan_workspace with the correct workspaceId', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'rescan_workspace') return { added: [], deactivated: [], unchanged: ['MyRepo'] }
      return null
    })
    render(<RepositoriesBlock projectId="proj-42" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Rescan workspace/i })))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('rescan_workspace', { workspaceId: 'proj-42' })
    )
  })

  it('shows "All repos up to date." when rescan finds no changes', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'rescan_workspace') return { added: [], deactivated: [], unchanged: ['MyRepo'] }
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Rescan workspace/i })))
    await waitFor(() => expect(screen.getByText(/All repos up to date/i)).toBeInTheDocument())
  })

  it('shows added count when rescan finds new repos', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'rescan_workspace') return { added: ['new-repo'], deactivated: [], unchanged: ['MyRepo'] }
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Rescan workspace/i })))
    await waitFor(() => expect(screen.getByText(/Added: 1/i)).toBeInTheDocument())
  })

  it('shows deactivated count when repos go missing', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'rescan_workspace') return { added: [], deactivated: ['gone-repo'], unchanged: [] }
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Rescan workspace/i })))
    await waitFor(() => expect(screen.getByText(/No longer found: 1/i)).toBeInTheDocument())
  })
})

// ── 22.3.22a: permanent voice guide hint in settings ─────────────────────────

describe('22.3.22a: permanent voice guide hint', () => {
  it('shows voice guide hint when get_workspace_path returns a path', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'get_workspace_path') return '/Users/hugo/code/myorg'
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => {
      const hint = screen.getByTestId('voice-guide-hint')
      expect(hint).toBeInTheDocument()
      expect(hint.getAttribute('data-workspace-path')).toBe('/Users/hugo/code/myorg')
    })
  })

  it('does not show voice guide hint when no workspace path returned', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'get_workspace_path') return null
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.queryByTestId('voice-guide-hint')).not.toBeInTheDocument())
  })

  it('shows creation hint with dismiss over permanent hint after workspace creation', async () => {
    setupSingleRepoMock()
    mockOpen.mockResolvedValue('/Users/hugo/code/myorg')
    render(<RepositoriesBlock projectId="proj-42" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Add workspace/i })))
    await waitFor(() => {
      const hint = screen.getByTestId('voice-guide-hint')
      // The creation hint is shown — it has a dismiss button via the mock
      expect(hint.getAttribute('data-workspace-path')).toBe('/Users/hugo/code/myorg')
    })
  })
})
