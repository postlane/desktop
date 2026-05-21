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

  it('shows standard empty-state message when no GitHub App repos and no folder repos', async () => {
    mockUseProjectRepos.mockReturnValue({ repos: [], loadError: null, refresh: mockRefresh })
    // list_github_app_repos returns null (unknown command) → treated as empty
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByText(/No repositories connected/i)).toBeInTheDocument()
    )
    expect(screen.queryByText(/monitored via your github app/i)).not.toBeInTheDocument()
  })

  it('calls list_github_app_repos with the correct projectId', async () => {
    mockUseProjectRepos.mockReturnValue({ repos: [], loadError: null, refresh: mockRefresh })
    render(<RepositoriesBlock projectId="proj-42" projectName="Test Org" isOwner={true} />)
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('list_github_app_repos', { projectId: 'proj-42' })
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

// ── GitHub App repos section (21.9.5–21.9.9, 21.9.13–21.9.16) ────────────────

function makeGitHubAppRepo(overrides = {}) {
  return { id: 1, name: 'org-repo', full_name: 'org/org-repo', private: false, html_url: 'https://github.com/org/org-repo', ...overrides }
}

function withAppRepo(overrides = {}) {
  return async (cmd: string) => {
    if (cmd === 'list_github_app_repos') return [makeGitHubAppRepo(overrides)]
    return null
  }
}

describe('RepositoriesBlock — GitHub App repos rendering', () => {
  it('renders a read-only GitHub App section when app-repos are returned', async () => {
    mockInvoke.mockImplementation(withAppRepo())
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('GitHub App')).toBeInTheDocument())
  })

  it('shows repo name and full_name in the GitHub App section', async () => {
    mockInvoke.mockImplementation(withAppRepo({ name: 'my-repo', full_name: 'myorg/my-repo' }))
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('myorg/my-repo')).toBeInTheDocument())
  })

  it('renders a link to html_url for each GitHub App repo', async () => {
    mockInvoke.mockImplementation(withAppRepo({ html_url: 'https://github.com/org/org-repo' }))
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => {
      const link = screen.getByRole('link', { name: /github\.com\/org\/org-repo/i })
      expect(link).toHaveAttribute('href', 'https://github.com/org/org-repo')
    })
  })

  it('shows folder repos section alongside GitHub App repos section', async () => {
    mockInvoke.mockImplementation(withAppRepo())
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('GitHub App')).toBeInTheDocument())
    expect(screen.getByText('MyRepo')).toBeInTheDocument()
  })

  it('suppresses old stopgap "monitored via" text when GitHub App repos are shown', async () => {
    mockUseProjectRepos.mockReturnValue({ repos: [], loadError: null, refresh: mockRefresh })
    mockInvoke.mockImplementation(withAppRepo())
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('GitHub App')).toBeInTheDocument())
    expect(screen.queryByText(/monitored via your github app/i)).not.toBeInTheDocument()
  })

  it('shows Add repository button even when GitHub App repos are present', async () => {
    mockUseProjectRepos.mockReturnValue({ repos: [], loadError: null, refresh: mockRefresh })
    mockInvoke.mockImplementation(withAppRepo())
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('GitHub App')).toBeInTheDocument())
    expect(screen.getByRole('button', { name: /Add repository/i })).toBeInTheDocument()
  })

  it('does not render GitHub App section when no app repos are returned', async () => {
    mockInvoke.mockImplementation(async () => [])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.queryByText('GitHub App')).not.toBeInTheDocument())
  })
})

describe('RepositoriesBlock — GitHub App deduplication', () => {
  function withDedupedRepo() {
    return async (cmd: string) => {
      if (cmd === 'list_github_app_repos') return [makeGitHubAppRepo({ name: 'org-repo', full_name: 'org/org-repo' })]
      return null
    }
  }

  it('shows local folder linked indicator when repo appears in both lists', async () => {
    mockUseProjectRepos.mockReturnValue({
      repos: [makeRepo({ name: 'org-repo', path: '/repos/org-repo' })],
      loadError: null,
      refresh: mockRefresh,
    })
    mockInvoke.mockImplementation(withDedupedRepo())
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('org/org-repo')).toBeInTheDocument())
    expect(screen.getByText(/local folder linked/i)).toBeInTheDocument()
  })

  it('suppresses deduplicated repo from the folder section', async () => {
    mockUseProjectRepos.mockReturnValue({
      repos: [makeRepo({ name: 'org-repo', path: '/repos/org-repo' })],
      loadError: null,
      refresh: mockRefresh,
    })
    mockInvoke.mockImplementation(withDedupedRepo())
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('org/org-repo')).toBeInTheDocument())
    expect(screen.getAllByText('org-repo')).toHaveLength(1)
  })
})

describe('RepositoriesBlock — no Configure button', () => {
  it('does not render a Configure button for owners', () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    expect(screen.queryByRole('button', { name: /Configure/i })).not.toBeInTheDocument()
  })

  it('does not render a Configure button for non-owners', () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={false} />)
    expect(screen.queryByRole('button', { name: /Configure/i })).not.toBeInTheDocument()
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

// ── Disconnect GitHub App (21.9.21) ───────────────────────────────────────────

describe('RepositoriesBlock — Disconnect GitHub App (21.9.21)', () => {
  it('shows Disconnect button for owner when app repos are present', async () => {
    mockInvoke.mockImplementation(withAppRepo())
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('GitHub App')).toBeInTheDocument())
    expect(screen.getByRole('button', { name: /Disconnect GitHub App/i })).toBeInTheDocument()
  })

  it('does not show Disconnect button for non-owners', async () => {
    mockInvoke.mockImplementation(withAppRepo())
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={false} />)
    await waitFor(() => expect(screen.getByText('GitHub App')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Disconnect GitHub App/i })).not.toBeInTheDocument()
  })

  it('does not show Disconnect button when no app repos', async () => {
    mockInvoke.mockImplementation(async () => [])
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.queryByText('GitHub App')).not.toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Disconnect GitHub App/i })).not.toBeInTheDocument()
  })

  it('shows confirmation prompt after clicking Disconnect', async () => {
    mockInvoke.mockImplementation(withAppRepo())
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('GitHub App')).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Disconnect GitHub App/i }))
    expect(screen.getByText(/This will remove Postlane/i)).toBeInTheDocument()
  })

  it('hides confirmation on Cancel', async () => {
    mockInvoke.mockImplementation(withAppRepo())
    render(<RepositoriesBlock projectId="proj-1" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('GitHub App')).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Disconnect GitHub App/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Cancel$/i }))
    expect(screen.queryByText(/This will remove Postlane/i)).not.toBeInTheDocument()
  })

  it('calls disconnect_github_app with projectId on confirm', async () => {
    mockInvoke.mockImplementation(withAppRepo())
    render(<RepositoriesBlock projectId="proj-42" projectName="Test Org" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('GitHub App')).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Disconnect GitHub App/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Confirm disconnect$/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('disconnect_github_app', { projectId: 'proj-42' })
    )
  })
})
