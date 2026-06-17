// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import type { Project, DraftPost, ViewSelection } from '../types'

vi.mock('../context/ProjectsProvider', () => ({
  useProjectsContext: vi.fn(),
}))
vi.mock('../context/DraftPostsProvider', () => ({
  useDraftPostsContext: vi.fn(),
}))
vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn().mockReturnValue({
    outerSize: vi.fn().mockResolvedValue({ width: 1100, height: 700 }),
    outerPosition: vi.fn().mockResolvedValue({ x: 0, y: 0 }),
  }),
}))

import { useProjectsContext } from '../context/ProjectsProvider'
import { useDraftPostsContext } from '../context/DraftPostsProvider'
import { invoke } from '../ipc/invoke'
import LeftNav from './LeftNav'
import UnassignedDraftBanner from '../components/UnassignedDraftBanner'

const mockUseProjectsCtx = vi.mocked(useProjectsContext)
const mockUseDraftPostsCtx = vi.mocked(useDraftPostsContext)
const mockInvoke = vi.mocked(invoke)

function makeProject(overrides: Partial<Project> = {}): Project {
  return {
    id: 'proj-1', name: 'Postlane', workspace_type: 'organization',
    tier: 'free', billing_active: true, is_owner: true,
    ...overrides,
  }
}

function makeDraft(overrides: Partial<DraftPost> = {}): DraftPost {
  return {
    repo_id: 'r1', repo_name: 'MyRepo', repo_path: '/path',
    post_folder: 'post-001', platforms: ['x'], platform: 'x',
    text: 'Hi', status: 'ready', trigger: null, error: null,
    image_url: null, project_id: 'proj-1', schedule: null,
    platform_results: null, llm_model: null, created_at: null,
    ...overrides,
  }
}

const DEFAULT_VIEW: ViewSelection = { view: 'no_orgs' }

function renderNav(props: Partial<Parameters<typeof LeftNav>[0]> = {}) {
  return render(
    <LeftNav
      currentView={DEFAULT_VIEW}
      onNavigate={vi.fn()}
      onSettingsOpen={vi.fn()}
      {...props}
    />,
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  mockUseProjectsCtx.mockReturnValue({
    projects: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
  })
  mockUseDraftPostsCtx.mockReturnValue({
    drafts: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
  })
  mockInvoke.mockResolvedValue(undefined)
})

// ── OrgItem rendering ────────────────────────────────────────────────────────

describe('LeftNav — OrgItem rendering', () => {
  it('shows one OrgItem per project', () => {
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ id: 'p1', name: 'Alpha' }), makeProject({ id: 'p2', name: 'Beta' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav()
    expect(screen.getByText('Alpha')).toBeInTheDocument()
    expect(screen.getByText('Beta')).toBeInTheDocument()
  })

  it('shows avatar with first letter of org name', () => {
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ name: 'Postlane' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav()
    expect(screen.getByLabelText(/workspace avatar for postlane/i)).toBeInTheDocument()
  })

  it('does not show billing warning icon regardless of billing_active', () => {
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ billing_active: false })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav()
    expect(screen.queryByLabelText(/billing inactive/i)).not.toBeInTheDocument()
  })
})

// ── Badge counting ───────────────────────────────────────────────────────────

describe('LeftNav — queue badge: collapsed org', () => {
  it('shows badge on org row when collapsed', () => {
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ id: 'proj-1', name: 'Acme' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    mockUseDraftPostsCtx.mockReturnValue({
      drafts: [makeDraft({ post_folder: 'post-1' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav()
    const orgBtn = screen.getByRole('button', { name: /acme/i })
    expect(orgBtn).toHaveTextContent('1')
  })

  it('does not show badge when count is zero', () => {
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ id: 'proj-1', name: 'Acme' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    mockUseDraftPostsCtx.mockReturnValue({
      drafts: [makeDraft({ project_id: 'other-org' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav()
    const orgBtn = screen.getByRole('button', { name: /acme/i })
    expect(orgBtn).not.toHaveTextContent('1')
  })
})

describe('LeftNav — queue badge: expanded org', () => {
  it('shows badge count next to Queue when org is expanded', () => {
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ id: 'proj-1', name: 'Acme' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    mockUseDraftPostsCtx.mockReturnValue({
      drafts: [makeDraft({ post_folder: 'post-1' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav()
    fireEvent.click(screen.getByText('Acme'))
    const queueBtn = screen.getByRole('button', { name: /queue/i })
    expect(queueBtn).toHaveTextContent('1')
  })

  it('hides badge from org row when expanded (badge moves to Queue)', () => {
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ id: 'proj-1', name: 'Acme' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    mockUseDraftPostsCtx.mockReturnValue({
      drafts: [makeDraft({ post_folder: 'post-1' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav()
    fireEvent.click(screen.getByText('Acme'))
    const orgBtn = screen.getByRole('button', { name: /acme/i })
    expect(orgBtn).not.toHaveTextContent('1')
  })

  it('counts unique (repo_path, post_folder) pairs — three platform rows = badge of 1', () => {
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ id: 'proj-1', name: 'Acme' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    mockUseDraftPostsCtx.mockReturnValue({
      drafts: [
        makeDraft({ repo_path: '/a', post_folder: 'post-1', platform: 'x' }),
        makeDraft({ repo_path: '/a', post_folder: 'post-1', platform: 'bluesky' }),
        makeDraft({ repo_path: '/a', post_folder: 'post-1', platform: 'mastodon' }),
      ],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav()
    fireEvent.click(screen.getByText('Acme'))
    expect(screen.getByText('1')).toBeInTheDocument()
  })
})

// ── Expand / sub-nav ─────────────────────────────────────────────────────────

describe('LeftNav — expand and sub-nav', () => {
  it('clicking org row expands it to show Queue, History, and Settings', () => {
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ id: 'proj-1', name: 'Acme' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav()
    fireEvent.click(screen.getByText('Acme'))
    expect(screen.getByText('Queue')).toBeInTheDocument()
    expect(screen.getByText('History')).toBeInTheDocument()
    expect(screen.getByText('Settings')).toBeInTheDocument()
  })

  it('clicking Settings navigates to org_settings view', () => {
    const onNavigate = vi.fn()
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ id: 'proj-1', name: 'Acme' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav({ onNavigate, currentView: { view: 'org_queue', projectId: 'proj-1' } })
    fireEvent.click(screen.getByText('Acme'))
    fireEvent.click(screen.getByText('Settings'))
    expect(onNavigate).toHaveBeenCalledWith({ view: 'org_settings', projectId: 'proj-1', section: 'queue' })
  })

  it('clicking Queue navigates to org_queue view', () => {
    const onNavigate = vi.fn()
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ id: 'proj-1', name: 'Acme' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav({ onNavigate, currentView: { view: 'org_queue', projectId: 'proj-1' } })
    fireEvent.click(screen.getByText('Acme'))
    fireEvent.click(screen.getByText('Queue'))
    expect(onNavigate).toHaveBeenCalledWith({ view: 'org_queue', projectId: 'proj-1' })
  })

  it('clicking History navigates to org_history view', () => {
    const onNavigate = vi.fn()
    mockUseProjectsCtx.mockReturnValue({
      projects: [makeProject({ id: 'proj-1', name: 'Acme' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav({ onNavigate, currentView: { view: 'org_queue', projectId: 'proj-1' } })
    fireEvent.click(screen.getByText('Acme'))
    fireEvent.click(screen.getByText('History'))
    expect(onNavigate).toHaveBeenCalledWith({ view: 'org_history', projectId: 'proj-1' })
  })
})

// ── Footer ───────────────────────────────────────────────────────────────────

describe('LeftNav — settings footer', () => {
  it('shows Account settings link', () => {
    renderNav()
    expect(screen.getByRole('button', { name: /account/i })).toBeInTheDocument()
  })

  it('shows Preferences settings link', () => {
    renderNav()
    expect(screen.getByRole('button', { name: /preferences/i })).toBeInTheDocument()
  })

  it('shows System settings link', () => {
    renderNav()
    expect(screen.getByRole('button', { name: /system/i })).toBeInTheDocument()
  })

  it('Cmd+, triggers onSettingsOpen', () => {
    const onSettingsOpen = vi.fn()
    renderNav({ onSettingsOpen })
    fireEvent.keyDown(document, { key: ',', metaKey: true })
    expect(onSettingsOpen).toHaveBeenCalledOnce()
  })

  it('clicking Account button calls onNavigate with global_settings account', () => {
    const onNavigate = vi.fn()
    renderNav({ onNavigate })
    fireEvent.click(screen.getByRole('button', { name: /account settings/i }))
    expect(onNavigate).toHaveBeenCalledWith({ view: 'global_settings', section: 'account' })
  })

  it('clicking Preferences button calls onNavigate with global_settings preferences', () => {
    const onNavigate = vi.fn()
    renderNav({ onNavigate })
    fireEvent.click(screen.getByRole('button', { name: /preferences settings/i }))
    expect(onNavigate).toHaveBeenCalledWith({ view: 'global_settings', section: 'preferences' })
  })

  it('clicking System button calls onNavigate with global_settings system', () => {
    const onNavigate = vi.fn()
    renderNav({ onNavigate })
    fireEvent.click(screen.getByRole('button', { name: /system settings/i }))
    expect(onNavigate).toHaveBeenCalledWith({ view: 'global_settings', section: 'system' })
  })
})

// ── Empty and error states ────────────────────────────────────────────────────

describe('LeftNav — empty and error states', () => {
  it('shows empty state message when no projects', () => {
    renderNav()
    expect(screen.getByText(/no workspaces/i)).toBeInTheDocument()
  })

  it('shows Add workspace button in empty state', () => {
    renderNav()
    expect(screen.getByRole('button', { name: /add.*workspace/i })).toBeInTheDocument()
  })

  it('Add workspace button is enabled', () => {
    renderNav()
    expect(screen.getByRole('button', { name: /add.*workspace/i })).not.toBeDisabled()
  })

  it('shows load error when projects.error is set', () => {
    mockUseProjectsCtx.mockReturnValue({
      projects: [], loading: false, error: 'Load failed', refresh: vi.fn(), clear: vi.fn(),
    })
    renderNav()
    expect(screen.getByText(/load failed/i)).toBeInTheDocument()
  })

  it('Retry button calls projects refresh on error', () => {
    const refresh = vi.fn()
    mockUseProjectsCtx.mockReturnValue({
      projects: [], loading: false, error: 'Load failed', refresh, clear: vi.fn(),
    })
    renderNav()
    fireEvent.click(screen.getByRole('button', { name: /retry/i }))
    expect(refresh).toHaveBeenCalledOnce()
  })
})

// ── UnassignedDraftBanner ─────────────────────────────────────────────────────

describe('UnassignedDraftBanner', () => {
  beforeEach(() => {
    mockInvoke.mockResolvedValue({
      version: 1, window: { width: 1100, height: 700, x: 0, y: 0 },
      nav: { last_view: 'no_orgs', last_repo_id: null, last_section: '', expanded_repos: [] },
      wizard_completed: true, timezone: '', telemetry_consent: false,
      consent_asked: true, default_post_time: null,
      dismissed_unassigned_draft_warning: false,
    })
  })

  it('shows banner when null-project-id drafts exist and flag is false', async () => {
    mockUseDraftPostsCtx.mockReturnValue({
      drafts: [makeDraft({ project_id: null })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    render(<UnassignedDraftBanner />)
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
  })

  it('hides banner when dismissed_unassigned_draft_warning is true', async () => {
    mockInvoke.mockResolvedValue({
      version: 1, window: { width: 1100, height: 700, x: 0, y: 0 },
      nav: { last_view: 'no_orgs', last_repo_id: null, last_section: '', expanded_repos: [] },
      wizard_completed: true, timezone: '', telemetry_consent: false,
      consent_asked: true, default_post_time: null,
      dismissed_unassigned_draft_warning: true,
    })
    mockUseDraftPostsCtx.mockReturnValue({
      drafts: [makeDraft({ project_id: null })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    render(<UnassignedDraftBanner />)
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument())
  })

  it('hides banner when all drafts have a project_id', async () => {
    mockUseDraftPostsCtx.mockReturnValue({
      drafts: [makeDraft({ project_id: 'proj-1' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    render(<UnassignedDraftBanner />)
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument())
  })

  it('dismiss button calls save_app_state_command and hides banner', async () => {
    mockUseDraftPostsCtx.mockReturnValue({
      drafts: [makeDraft({ project_id: null })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    render(<UnassignedDraftBanner />)
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith(
      'save_app_state_command',
      expect.objectContaining({ state: expect.objectContaining({ dismissed_unassigned_draft_warning: true }) }),
    ))
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument())
  })
})
