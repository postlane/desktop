// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('../wizard/AddRepoModal', () => ({
  default: ({ onClose, projectId, projectName }: { onClose: () => void; projectId: string; projectName: string }) => (
    <div data-testid="add-repo-modal" data-project-id={projectId} data-project-name={projectName}>
      <button onClick={onClose}>Close modal</button>
    </div>
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
import RepositoriesBlock from './RepositoriesBlock'

const mockInvoke = vi.mocked(invoke)

// ── Helpers ────────────────────────────────────────────────────────────────────

interface RepoConnectionStatus {
  repo_id: string | null
  github_full_name: string | null
  local_path: string | null
  display_name: string
  github_app_connected: boolean
  folder_registered: boolean
  cli_initialized: boolean
  project_id_mismatch: boolean
}

function makeRow(overrides: Partial<RepoConnectionStatus> = {}): RepoConnectionStatus {
  return {
    repo_id: 'repo-1',
    github_full_name: null,
    local_path: '/repos/myrepo',
    display_name: 'MyRepo',
    github_app_connected: false,
    folder_registered: true,
    cli_initialized: true,
    project_id_mismatch: false,
    ...overrides,
  }
}

function mockStatus(rows: RepoConnectionStatus[], extras: Record<string, unknown> = {}) {
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_repo_connection_status') return rows
    if (cmd in extras) return extras[cmd]
    return null
  })
}

beforeEach(() => {
  vi.clearAllMocks()
  mockStatus([makeRow()])
})

// ── Empty state ────────────────────────────────────────────────────────────────

describe('RepositoriesBlock — empty state', () => {
  it('shows empty-state message when no repos', async () => {
    mockStatus([])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText(/No repositories connected/i)).toBeInTheDocument())
  })

  it('shows Add workspace button for owners', async () => {
    mockStatus([])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('button', { name: /^Add workspace$/i })).toBeInTheDocument())
  })

  it('calls get_repo_connection_status with the correct projectId', async () => {
    render(<RepositoriesBlock projectId="proj-42" projectName="Test Org" isOwner={true} />)
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('get_repo_connection_status', { projectId: 'proj-42' })
    )
  })
})

// ── Table rendering ────────────────────────────────────────────────────────────

describe('RepositoriesBlock — table rendering', () => {
  it('renders display_name in the table', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('MyRepo')).toBeInTheDocument())
  })

  it('shows GitHub App column header', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('GitHub App')).toBeInTheDocument())
  })

  it('shows Folder column header', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('Folder')).toBeInTheDocument())
  })

  it('shows CLI column header', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('CLI')).toBeInTheDocument())
  })

  it('shows github_full_name when present', async () => {
    mockStatus([makeRow({ github_full_name: 'org/my-repo', github_app_connected: true })])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('org/my-repo')).toBeInTheDocument())
  })

  it('renders link to GitHub when github_full_name is set', async () => {
    mockStatus([makeRow({ github_full_name: 'org/my-repo', github_app_connected: true })])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => {
      const link = screen.getByRole('link')
      expect(link).toHaveAttribute('href', 'https://github.com/org/my-repo')
    })
  })

  it('shows warning icon when project_id_mismatch is true', async () => {
    mockStatus([makeRow({ cli_initialized: true, project_id_mismatch: true })])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByTitle(/different project/i)).toBeInTheDocument())
  })

  it('shows local_path in the row', async () => {
    mockStatus([makeRow({ local_path: '/Users/hugo/code/my-repo' })])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('/Users/hugo/code/my-repo')).toBeInTheDocument())
  })
})

// ── Remove ─────────────────────────────────────────────────────────────────────

describe('RepositoriesBlock — remove', () => {
  it('shows Remove button for folder_registered repos when isOwner', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('button', { name: /Remove/i })).toBeInTheDocument())
  })

  it('shows confirmation copy after first Remove click', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Remove/i })))
    expect(screen.getByText(/Existing drafts on disk are not deleted/i)).toBeInTheDocument()
  })

  it('calls unregister_repo with the correct repoId on confirm', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Remove/i })))
    fireEvent.click(screen.getByRole('button', { name: /^Confirm remove$/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('unregister_repo', { repoId: 'repo-1' })
    )
  })

  it('refreshes after successful remove', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Remove/i })))
    fireEvent.click(screen.getByRole('button', { name: /^Confirm remove$/i }))
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter((c) => c[0] === 'get_repo_connection_status')
      expect(calls.length).toBeGreaterThan(1)
    })
  })

  it('hides confirmation on Cancel', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Remove/i })))
    fireEvent.click(screen.getByRole('button', { name: /^Cancel$/i }))
    expect(screen.queryByText(/Existing drafts on disk are not deleted/i)).not.toBeInTheDocument()
  })

  it('does not show Remove button when repo has no repo_id', async () => {
    mockStatus([makeRow({ repo_id: null, folder_registered: false })])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('MyRepo')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Remove/i })).not.toBeInTheDocument()
  })
})

// ── Owner-only ─────────────────────────────────────────────────────────────────

describe('RepositoriesBlock — owner-only actions', () => {
  it('hides Remove button for non-owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={false} />)
    await waitFor(() => expect(screen.getByText('MyRepo')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Remove/i })).not.toBeInTheDocument()
  })

  it('hides Add workspace and Add individual repository for non-owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={false} />)
    await waitFor(() => expect(screen.getByText('MyRepo')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Add workspace/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Add individual repository/i })).not.toBeInTheDocument()
  })

  it('hides Scan for repos for non-owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={false} />)
    await waitFor(() => expect(screen.getByText('MyRepo')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Scan for repos/i })).not.toBeInTheDocument()
  })

  it('shows Scan for repos for owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('button', { name: /Scan for repos/i })).toBeInTheDocument())
  })
})

// ── Disconnect GitHub App ──────────────────────────────────────────────────────

describe('RepositoriesBlock — Disconnect GitHub App', () => {
  it('shows Disconnect button for owner when a row has github_app_connected=true', async () => {
    mockStatus([makeRow({ github_app_connected: true })])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Disconnect GitHub App/i })).toBeInTheDocument()
    )
  })

  it('hides Disconnect button when no rows have github_app_connected=true', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('MyRepo')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Disconnect GitHub App/i })).not.toBeInTheDocument()
  })

  it('hides Disconnect button for non-owners even when GitHub App connected', async () => {
    mockStatus([makeRow({ github_app_connected: true })])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={false} />)
    await waitFor(() => expect(screen.getByText('MyRepo')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Disconnect GitHub App/i })).not.toBeInTheDocument()
  })

  it('shows confirmation prompt after clicking Disconnect', async () => {
    mockStatus([makeRow({ github_app_connected: true })])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Disconnect GitHub App/i })))
    expect(screen.getByText(/This will remove Postlane/i)).toBeInTheDocument()
  })

  it('hides confirmation on Cancel', async () => {
    mockStatus([makeRow({ github_app_connected: true })])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Disconnect GitHub App/i })))
    fireEvent.click(screen.getByRole('button', { name: /^Cancel$/i }))
    expect(screen.queryByText(/This will remove Postlane/i)).not.toBeInTheDocument()
  })

  it('calls disconnect_github_app on confirm', async () => {
    mockStatus([makeRow({ github_app_connected: true })])
    render(<RepositoriesBlock projectId="proj-42" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Disconnect GitHub App/i })))
    fireEvent.click(screen.getByRole('button', { name: /^Confirm disconnect$/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('disconnect_github_app', { projectId: 'proj-42' })
    )
  })
})

// ── Scan for repos ─────────────────────────────────────────────────────────────

describe('RepositoriesBlock — Scan for repos', () => {
  const emptyResult = { added: [], already_registered: [], not_found_on_disk: [], failed_to_register: [] }

  it('calls discover_repos with projectId when button clicked', async () => {
    mockStatus([makeRow()], { discover_repos: emptyResult })
    render(<RepositoriesBlock projectId="proj-42" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Scan for repos/i })))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('discover_repos', { projectId: 'proj-42' })
    )
  })

  it('shows added repos in scan result', async () => {
    mockStatus([makeRow()], { discover_repos: { added: ['new-repo'], already_registered: [], not_found_on_disk: [], failed_to_register: [] } })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Scan for repos/i })))
    await waitFor(() => expect(screen.getByText(/new-repo/)).toBeInTheDocument())
  })

  it('shows not_found_on_disk repos with Add folder manually button', async () => {
    mockStatus([makeRow()], { discover_repos: { added: [], already_registered: [], not_found_on_disk: ['org/missing'], failed_to_register: [] } })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Scan for repos/i })))
    await waitFor(() => expect(screen.getByText(/org\/missing/)).toBeInTheDocument())
    expect(screen.getByRole('button', { name: /Add folder manually/i })).toBeInTheDocument()
  })

  it('shows error for failed_to_register entries', async () => {
    mockStatus([makeRow()], { discover_repos: { added: [], already_registered: [], not_found_on_disk: [], failed_to_register: [['/some/path', 'permission denied']] } })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Scan for repos/i })))
    await waitFor(() => expect(screen.getByText(/permission denied/)).toBeInTheDocument())
  })
})

// ── AddRepoModal (via "Add individual repository") ────────────────────────────

describe('RepositoriesBlock — AddRepoModal', () => {
  it('opens modal when "Add individual repository" is clicked', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Add individual repository/i })))
    expect(screen.getByTestId('add-repo-modal')).toBeInTheDocument()
  })

  it('refreshes when modal closes', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Add individual repository/i })))
    fireEvent.click(screen.getByRole('button', { name: /Close modal/i }))
    const calls = mockInvoke.mock.calls.filter((c) => c[0] === 'get_repo_connection_status')
    expect(calls.length).toBeGreaterThan(1)
  })

  it('passes projectId to AddRepoModal', async () => {
    render(<RepositoriesBlock projectId="proj-42" projectName="Test Org" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Add individual repository/i })))
    expect(screen.getByTestId('add-repo-modal')).toHaveAttribute('data-project-id', 'proj-42')
  })

  it('passes projectName to AddRepoModal', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="my-workspace" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Add individual repository/i })))
    expect(screen.getByTestId('add-repo-modal')).toHaveAttribute('data-project-name', 'my-workspace')
  })
})

// ── No Configure button ────────────────────────────────────────────────────────

describe('RepositoriesBlock — no Configure button', () => {
  it('does not render a Configure button for owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('MyRepo')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Configure/i })).not.toBeInTheDocument()
  })

  it('does not render a Configure button for non-owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={false} />)
    await waitFor(() => expect(screen.getByText('MyRepo')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Configure/i })).not.toBeInTheDocument()
  })
})
