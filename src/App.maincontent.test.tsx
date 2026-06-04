// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('./ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }))
vi.mock('./context/ProjectsProvider', () => ({
  ProjectsProvider: vi.fn(),
  useProjectsContext: vi.fn(),
}))
vi.mock('./context/DraftPostsProvider', () => ({
  DraftPostsProvider: vi.fn(),
  useDraftPostsContext: vi.fn(),
}))
vi.mock('./hooks/useSentPosts', () => ({ useSentPosts: vi.fn() }))
vi.mock('./components/PostTable', () => ({
  default: ({ onSelect }: { onSelect?: (_p: unknown) => void }) => (
    <div data-testid="post-table">
      <button data-testid="select-post" onClick={() => onSelect?.({
        id: 'd1', project_id: 'p1', repo_id: 'r1', title: 'T', body: '', status: 'draft', created_at: '',
      })}>Select</button>
    </div>
  ),
}))
vi.mock('./components/EditPostView', () => ({
  default: ({ onDirtyChange, onBack, onApproved }: {
    onDirtyChange?: (_d: boolean) => void;
    onBack?: () => void;
    onApproved?: () => void;
  }) => (
    <div data-testid="edit-post-view">
      <button data-testid="set-dirty" onClick={() => onDirtyChange?.(true)}>Dirty</button>
      <button data-testid="post-back" onClick={onBack}>Back</button>
      <button data-testid="post-approved" onClick={onApproved}>Approved</button>
    </div>
  ),
}))
vi.mock('./settings/OrgSettingsView', () => ({
  default: ({ onDisconnected, onDeleted }: { onDisconnected?: () => void; onDeleted?: () => void }) => (
    <div>OrgSettingsView
      <button data-testid="trigger-disconnected" onClick={onDisconnected}>Disconnected</button>
      <button data-testid="trigger-deleted" onClick={onDeleted}>Deleted</button>
    </div>
  ),
}))
vi.mock('./settings/AccountSettingsView', () => ({ default: () => <div>AccountSettingsView</div> }))
vi.mock('./settings/PreferencesSettingsView', () => ({ default: () => <div>PreferencesSettingsView</div> }))
vi.mock('./settings/SystemSettingsView', () => ({ default: () => <div>SystemSettingsView</div> }))
vi.mock('./components/OrgUpgradeBanner', () => ({
  default: ({ onConnect }: { onConnect?: () => void }) => (
    <button data-testid="org-connect" onClick={onConnect}>Connect</button>
  ),
}))
vi.mock('./components/OrgLinkModal', () => ({
  default: ({ onDone, onClose }: { onDone?: (_login: string) => void; onClose?: () => void }) => (
    <>
      <button data-testid="org-link-done" onClick={() => onDone?.('my-org')}>Done</button>
      <button data-testid="org-link-close" onClick={onClose}>Close</button>
    </>
  ),
}))
vi.mock('./settings/WorkspaceMissingBanner', () => ({
  default: () => null,
  useWorkspaceStatus: () => ({ result: null, clearStatus: vi.fn() }),
}))

vi.mock('./settings/MigrationBanner', () => ({
  MigrationBannerContent: () => null,
  RecoveryBannerContent: () => null,
  MigrationBannersBlock: () => null,
  useMigrationStatus: () => ({ status: null, dismiss: vi.fn() }),
  useJournalStatuses: () => ({ statuses: [], resume: vi.fn(), dismissSession: vi.fn() }),
}))

import { MainContent } from './App'
import { useProjectsContext } from './context/ProjectsProvider'
import { useDraftPostsContext } from './context/DraftPostsProvider'
import { useSentPosts } from './hooks/useSentPosts'
import type { ViewSelection, Project } from './types'

const mockUseProjectsContext = vi.mocked(useProjectsContext)
const mockUseDraftPostsContext = vi.mocked(useDraftPostsContext)
const mockUseSentPosts = vi.mocked(useSentPosts)

const MOCK_PROJECT: Project = {
  id: 'p1', name: 'My Org', workspace_type: 'personal', tier: 'free', billing_active: true, is_owner: true,
}

function baseProps(view: ViewSelection = { view: 'no_orgs' }) {
  return {
    view,
    onNavigate: vi.fn(),
    onToast: vi.fn(),
    onDirtyChange: vi.fn(),
    onTimezoneChange: vi.fn(),
    onRepoChange: vi.fn(),
    onSignedOut: vi.fn(),
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  mockUseProjectsContext.mockReturnValue({ projects: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
  mockUseDraftPostsContext.mockReturnValue({ drafts: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
  mockUseSentPosts.mockReturnValue({ posts: [], loading: false, error: null, refresh: vi.fn() })
})

// ── org_queue: draft states ───────────────────────────────────────────────────

describe('MainContent — org_queue draft states', () => {
  it('renders PostTable when drafts loaded', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'p1' })} />)
    expect(screen.getByTestId('post-table')).toBeInTheDocument()
  })

  it('shows LoadingView while drafts loading', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    mockUseDraftPostsContext.mockReturnValue({ drafts: [], loading: true, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'p1' })} />)
    expect(screen.getByText('Loading…')).toBeInTheDocument()
  })

  it('shows QueueLoadError when drafts fetch fails', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    mockUseDraftPostsContext.mockReturnValue({ drafts: [], loading: false, error: 'IPC failure', refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'p1' })} />)
    expect(screen.getByText('IPC failure')).toBeInTheDocument()
  })

  it('Retry in queue error state calls refresh', () => {
    const mockRefresh = vi.fn()
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    mockUseDraftPostsContext.mockReturnValue({ drafts: [], loading: false, error: 'fail', refresh: mockRefresh, clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'p1' })} />)
    fireEvent.click(screen.getByRole('button', { name: /retry/i }))
    expect(mockRefresh).toHaveBeenCalledOnce()
  })
})

// ── org_queue: post selection ─────────────────────────────────────────────────

describe('MainContent — org_queue post selection', () => {
  it('selecting a post renders EditPostView', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'p1' })} />)
    fireEvent.click(screen.getByTestId('select-post'))
    expect(screen.getByTestId('edit-post-view')).toBeInTheDocument()
  })

  it('back from EditPostView returns to PostTable', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'p1' })} />)
    fireEvent.click(screen.getByTestId('select-post'))
    fireEvent.click(screen.getByTestId('post-back'))
    expect(screen.getByTestId('post-table')).toBeInTheDocument()
  })

  it('approved from EditPostView calls refresh and returns to PostTable', () => {
    const mockRefresh = vi.fn()
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    mockUseDraftPostsContext.mockReturnValue({ drafts: [], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'p1' })} />)
    fireEvent.click(screen.getByTestId('select-post'))
    fireEvent.click(screen.getByTestId('post-approved'))
    expect(screen.getByTestId('post-table')).toBeInTheDocument()
    expect(mockRefresh).toHaveBeenCalled()
  })
})

// ── org_queue: org link modal ─────────────────────────────────────────────────

describe('MainContent — org_queue org link modal', () => {
  it('clicking OrgUpgradeBanner connect shows org link modal', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'p1' })} />)
    fireEvent.click(screen.getByTestId('org-connect'))
    expect(screen.getByText('Connect GitHub org')).toBeInTheDocument()
  })

  it('clicking modal background closes org link modal', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'p1' })} />)
    fireEvent.click(screen.getByTestId('org-connect'))
    const background = document.querySelector('.modal-background')
    expect(background).not.toBeNull()
    if (background) fireEvent.click(background)
    expect(screen.queryByText('Connect GitHub org')).not.toBeInTheDocument()
  })

  it('OrgLink onDone closes modal and calls onToast', () => {
    const onToast = vi.fn()
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'p1' })} onToast={onToast} />)
    fireEvent.click(screen.getByTestId('org-connect'))
    fireEvent.click(screen.getByTestId('org-link-done'))
    expect(onToast).toHaveBeenCalledWith('GitHub org connected.')
    expect(screen.queryByText('Connect GitHub org')).not.toBeInTheDocument()
  })

  it('OrgLink onClose closes the modal', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'p1' })} />)
    fireEvent.click(screen.getByTestId('org-connect'))
    expect(screen.getByText('Connect GitHub org')).toBeInTheDocument()
    fireEvent.click(screen.getByTestId('org-link-close'))
    expect(screen.queryByText('Connect GitHub org')).not.toBeInTheDocument()
  })
})

// ── org_history ───────────────────────────────────────────────────────────────

describe('MainContent — org_history', () => {
  it('shows LoadingView while history loading', () => {
    mockUseSentPosts.mockReturnValue({ posts: [], loading: true, error: null, refresh: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_history', projectId: 'p1' })} />)
    expect(screen.getByText('Loading…')).toBeInTheDocument()
  })

  it('shows QueueLoadError when history fetch fails', () => {
    const mockRefresh = vi.fn()
    mockUseSentPosts.mockReturnValue({ posts: [], loading: false, error: 'Network error', refresh: mockRefresh })
    render(<MainContent {...baseProps({ view: 'org_history', projectId: 'p1' })} />)
    expect(screen.getByText('Network error')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /retry/i }))
    expect(mockRefresh).toHaveBeenCalledOnce()
  })

  it('renders PostTable when history loaded', () => {
    render(<MainContent {...baseProps({ view: 'org_history', projectId: 'p1' })} />)
    expect(screen.getByTestId('post-table')).toBeInTheDocument()
  })
})

// ── org_history: post selection ───────────────────────────────────────────────

describe('MainContent — org_history post selection', () => {
  it('selecting a history post renders EditPostView', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_history', projectId: 'p1' })} />)
    fireEvent.click(screen.getByTestId('select-post'))
    expect(screen.getByTestId('edit-post-view')).toBeInTheDocument()
  })

  it('back from history EditPostView returns to PostTable', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_history', projectId: 'p1' })} />)
    fireEvent.click(screen.getByTestId('select-post'))
    fireEvent.click(screen.getByTestId('post-back'))
    expect(screen.getByTestId('post-table')).toBeInTheDocument()
  })

  it('history EditPostView does not open when project not found', () => {
    render(<MainContent {...baseProps({ view: 'org_history', projectId: 'p1' })} />)
    fireEvent.click(screen.getByTestId('select-post'))
    expect(screen.queryByTestId('edit-post-view')).not.toBeInTheDocument()
    expect(screen.getByTestId('post-table')).toBeInTheDocument()
  })
})

// ── org_settings ──────────────────────────────────────────────────────────────

describe('MainContent — org_settings', () => {
  it('shows LoadingView when project not found', () => {
    render(<MainContent {...baseProps({ view: 'org_settings', projectId: 'missing', section: 'queue' })} />)
    expect(screen.getByText('Loading…')).toBeInTheDocument()
  })

  it('renders OrgSettingsView when project found', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_settings', projectId: 'p1', section: 'queue' })} />)
    expect(screen.getByText('OrgSettingsView')).toBeInTheDocument()
  })
})

// ── global_settings ───────────────────────────────────────────────────────────

describe('MainContent — global_settings', () => {
  it('renders AccountSettingsView for account section', () => {
    render(<MainContent {...baseProps({ view: 'global_settings', section: 'account' })} />)
    expect(screen.getByText('AccountSettingsView')).toBeInTheDocument()
  })

  it('renders PreferencesSettingsView for preferences section', () => {
    render(<MainContent {...baseProps({ view: 'global_settings', section: 'preferences' })} />)
    expect(screen.getByText('PreferencesSettingsView')).toBeInTheDocument()
  })

  it('renders SystemSettingsView for system section', () => {
    render(<MainContent {...baseProps({ view: 'global_settings', section: 'system' })} />)
    expect(screen.getByText('SystemSettingsView')).toBeInTheDocument()
  })
})

// ── org_queue: project not found ─────────────────────────────────────────────

describe('MainContent — org_queue project not in list', () => {
  it('renders PostTable even when the project is not in the list', () => {
    // projects list is empty so find returns undefined (the ?? null branch)
    mockUseProjectsContext.mockReturnValue({ projects: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_queue', projectId: 'unknown' })} />)
    expect(screen.getByTestId('post-table')).toBeInTheDocument()
  })
})

// ── no_orgs auto-navigation ───────────────────────────────────────────────────

describe('MainContent — no_orgs', () => {
  it('shows LoadingView when projects still loading', () => {
    mockUseProjectsContext.mockReturnValue({ projects: [], loading: true, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps()} />)
    expect(screen.getByText('Loading…')).toBeInTheDocument()
  })

  it('auto-navigates to org_queue when projects load', async () => {
    const onNavigate = vi.fn()
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(<MainContent {...baseProps()} onNavigate={onNavigate} />)
    await waitFor(() => expect(onNavigate).toHaveBeenCalledWith({ view: 'org_queue', projectId: 'p1' }))
  })

  it('auto-navigates to org_settings when wizard nudge pending', async () => {
    const onNavigate = vi.fn()
    const onWizardNudgeHandled = vi.fn()
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    render(
      <MainContent {...baseProps()} onNavigate={onNavigate}
        wizardNudgePending={true} onWizardNudgeHandled={onWizardNudgeHandled} />,
    )
    await waitFor(() => expect(onNavigate).toHaveBeenCalledWith({ view: 'org_settings', projectId: 'p1', section: 'queue' }))
    expect(onWizardNudgeHandled).toHaveBeenCalled()
  })
})

// ── org_settings: post-disconnect navigation (22.10.14) ───────────────────────

describe('MainContent — org_settings post-disconnect navigation (22.10.14)', () => {
  it('navigates to no_orgs when workspace is disconnected', async () => {
    const onNavigate = vi.fn()
    const refreshProjects = vi.fn()
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: refreshProjects, clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_settings', projectId: 'p1', section: 'settings' })} onNavigate={onNavigate} />)
    fireEvent.click(screen.getByTestId('trigger-disconnected'))
    expect(onNavigate).toHaveBeenCalledWith({ view: 'no_orgs' })
  })

  it('refreshes projects when workspace is disconnected', async () => {
    const onNavigate = vi.fn()
    const refreshProjects = vi.fn()
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: refreshProjects, clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_settings', projectId: 'p1', section: 'settings' })} onNavigate={onNavigate} />)
    fireEvent.click(screen.getByTestId('trigger-disconnected'))
    expect(refreshProjects).toHaveBeenCalled()
  })
})

// ── org_settings: post-delete navigation (22.10.15) ───────────────────────────

describe('MainContent — org_settings post-delete navigation (22.10.15)', () => {
  it('navigates to no_orgs when workspace is deleted', async () => {
    const onNavigate = vi.fn()
    const refreshProjects = vi.fn()
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: refreshProjects, clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_settings', projectId: 'p1', section: 'settings' })} onNavigate={onNavigate} />)
    fireEvent.click(screen.getByTestId('trigger-deleted'))
    expect(onNavigate).toHaveBeenCalledWith({ view: 'no_orgs' })
  })

  it('refreshes projects when workspace is deleted', async () => {
    const onNavigate = vi.fn()
    const refreshProjects = vi.fn()
    mockUseProjectsContext.mockReturnValue({ projects: [MOCK_PROJECT], loading: false, error: null, refresh: refreshProjects, clear: vi.fn() })
    render(<MainContent {...baseProps({ view: 'org_settings', projectId: 'p1', section: 'settings' })} onNavigate={onNavigate} />)
    fireEvent.click(screen.getByTestId('trigger-deleted'))
    expect(refreshProjects).toHaveBeenCalled()
  })
})
