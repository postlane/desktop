// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('../hooks/useRepoData', () => ({ useProjectRepos: vi.fn() }))
vi.mock('../wizard/AddRepoModal', () => ({
  default: ({ onClose, projectId, projectName }: { onClose: () => void; projectId: string; projectName: string }) => (
    <div data-testid="add-repo-modal" data-project-id={projectId} data-project-name={projectName}>
      <button onClick={onClose}>Close modal</button>
    </div>
  ),
}))

import { invoke } from '../ipc/invoke'
import { useProjectRepos } from '../hooks/useRepoData'
import RepositoriesBlock from './RepositoriesBlock'

const mockInvoke = vi.mocked(invoke)
const mockUseProjectRepos = vi.mocked(useProjectRepos)
const mockRefresh = vi.fn()

function makeRepo(overrides = {}) {
  return { id: 'repo-1', name: 'MyRepo', path: '/repos/myrepo', active: true, ...overrides }
}

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockResolvedValue(null)
  mockUseProjectRepos.mockReturnValue({ repos: [makeRepo()], loadError: null, refresh: mockRefresh })
})

// ── Empty state ────────────────────────────────────────────────────────────────

describe('RepositoriesBlock — empty state', () => {
  it('shows empty-state message when no repos', () => {
    mockUseProjectRepos.mockReturnValue({ repos: [], loadError: null, refresh: mockRefresh })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    expect(screen.getByText(/No repositories connected/i)).toBeInTheDocument()
  })

  it('shows add button in empty state for owner', () => {
    mockUseProjectRepos.mockReturnValue({ repos: [], loadError: null, refresh: mockRefresh })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    expect(screen.getByRole('button', { name: /Add repository/i })).toBeInTheDocument()
  })

  it('shows GitHub App note when app is installed and no repos are folder-connected', async () => {
    mockUseProjectRepos.mockReturnValue({ repos: [], loadError: null, refresh: mockRefresh })
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return true
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByText(/monitored via your github app/i)).toBeInTheDocument()
    )
    expect(screen.queryByText(/No repositories connected/i)).not.toBeInTheDocument()
  })

  it('still shows standard message when GitHub App check returns false', async () => {
    mockUseProjectRepos.mockReturnValue({ repos: [], loadError: null, refresh: mockRefresh })
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return false
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() =>
      expect(screen.queryByText(/monitored via your github app/i)).not.toBeInTheDocument()
    )
    expect(screen.getByText(/No repositories connected/i)).toBeInTheDocument()
  })

  it('does not show GitHub App note when repos are present even if app is installed', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_github_app_installed') return true
      return null
    })
    // repos is non-empty (beforeEach default)
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() =>
      expect(screen.queryByText(/monitored via your github app/i)).not.toBeInTheDocument()
    )
  })

  it('calls check_github_app_installed with the correct projectId', async () => {
    mockUseProjectRepos.mockReturnValue({ repos: [], loadError: null, refresh: mockRefresh })
    render(<RepositoriesBlock projectId="proj-42" projectName="Test Org" isOwner={true} />)
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('check_github_app_installed', { projectId: 'proj-42' })
    )
  })
})

// ── Repo list ──────────────────────────────────────────────────────────────────

describe('RepositoriesBlock — repo list', () => {
  it('renders repo name', () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    expect(screen.getByText('MyRepo')).toBeInTheDocument()
  })

  it('calls useProjectRepos with the given projectId', () => {
    render(<RepositoriesBlock projectId="proj-42" projectName="Test Org" isOwner={true} />)
    expect(mockUseProjectRepos).toHaveBeenCalledWith('proj-42')
  })
})

// ── Remove ─────────────────────────────────────────────────────────────────────

describe('RepositoriesBlock — remove', () => {
  it('shows confirmation copy after first Remove click', () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    fireEvent.click(screen.getByRole('button', { name: /Remove/i }))
    expect(screen.getByText(/Existing drafts on disk are not deleted/i)).toBeInTheDocument()
  })

  it('calls unregister_repo on confirm click', async () => {
    mockInvoke.mockResolvedValue(undefined)
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    fireEvent.click(screen.getByRole('button', { name: /Remove/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Confirm remove$/i }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('unregister_repo', { repoId: 'repo-1' }))
  })

  it('calls refresh after successful remove', async () => {
    mockInvoke.mockResolvedValue(undefined)
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    fireEvent.click(screen.getByRole('button', { name: /Remove/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Confirm remove$/i }))
    await waitFor(() => expect(mockRefresh).toHaveBeenCalled())
  })

  it('cancels remove confirmation on Cancel click', () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    fireEvent.click(screen.getByRole('button', { name: /Remove/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Cancel$/i }))
    expect(screen.queryByText(/Existing drafts on disk are not deleted/i)).not.toBeInTheDocument()
  })
})

// ── Owner-only actions ─────────────────────────────────────────────────────────

describe('RepositoriesBlock — owner-only', () => {
  it('hides Remove button for non-owners', () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={false} />)
    expect(screen.queryByRole('button', { name: /Remove/i })).not.toBeInTheDocument()
  })

  it('hides Add repository button for non-owners', () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={false} />)
    expect(screen.queryByRole('button', { name: /Add repository/i })).not.toBeInTheDocument()
  })
})

// ── AddRepoModal ───────────────────────────────────────────────────────────────

describe('RepositoriesBlock — AddRepoModal', () => {
  it('opens AddRepoModal when Add repository is clicked', () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    fireEvent.click(screen.getByRole('button', { name: /Add repository/i }))
    expect(screen.getByTestId('add-repo-modal')).toBeInTheDocument()
  })

  it('calls refresh when AddRepoModal closes', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    fireEvent.click(screen.getByRole('button', { name: /Add repository/i }))
    fireEvent.click(screen.getByRole('button', { name: /Close modal/i }))
    expect(mockRefresh).toHaveBeenCalled()
  })

  it('passes the projectId to AddRepoModal (Security Rule 2: repo must be linked to the correct project)', () => {
    render(<RepositoriesBlock projectId="proj-42" projectName="Test Org" isOwner={true} />)
    fireEvent.click(screen.getByRole('button', { name: /Add repository/i }))
    expect(screen.getByTestId('add-repo-modal')).toHaveAttribute('data-project-id', 'proj-42')
  })

  it('passes projectName to AddRepoModal so already-connected errors can name the workspace', () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="my-workspace" isOwner={true} />)
    fireEvent.click(screen.getByRole('button', { name: /Add repository/i }))
    expect(screen.getByTestId('add-repo-modal')).toHaveAttribute('data-project-name', 'my-workspace')
  })
})
