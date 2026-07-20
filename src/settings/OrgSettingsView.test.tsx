// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import '@testing-library/jest-dom'
import { MantineProvider } from '@mantine/core'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('../hooks/useRepoData', () => ({ useProjectRepos: vi.fn() }))
vi.mock('../context/ProjectsProvider', () => ({ useProjectsContext: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))

import { invoke } from '../ipc/invoke'
import { useProjectRepos } from '../hooks/useRepoData'
import { useProjectsContext } from '../context/ProjectsProvider'
import OrgSettingsView from './OrgSettingsView'
import type { Project } from '../types'

const mockInvoke = vi.mocked(invoke)
const mockUseProjectRepos = vi.mocked(useProjectRepos)
const mockCtx = vi.mocked(useProjectsContext)

function makeProject(overrides: Partial<Project> = {}): Project {
  return { id: 'proj-1', name: 'Acme', workspace_type: 'organization', tier: 'free', billing_active: true, is_owner: true, ...overrides }
}

// OrgSettingsView renders BillingBlock, which renders WithdrawFromContractButton
// (checklist 24.4.13) -- a Mantine component -- so every render needs a
// MantineProvider now.
function renderView(org: Project) {
  return render(
    <MantineProvider>
      <OrgSettingsView org={org} />
    </MantineProvider>,
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'get_repo_connection_status') return []
    if (cmd === 'list_connected_providers') return []
    if (cmd === 'get_scheduler_account_names') return {}
    if (cmd === 'list_scheduler_profiles') return []
    if (cmd === 'get_project_voice_guide') return ''
    return null
  })
  mockUseProjectRepos.mockReturnValue({ repos: [], loadError: null, refresh: vi.fn() })
  mockCtx.mockReturnValue({ projects: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
})

describe('OrgSettingsView', () => {
  it('renders RepositoriesBlock', async () => {
    renderView(makeProject())
    await screen.findByText(/No repositories connected/i)
  })

  it('renders SchedulerBlock', () => {
    renderView(makeProject())
    expect(screen.getByText('Scheduler')).toBeInTheDocument()
  })

  it('renders VoiceGuideBlock', () => {
    renderView(makeProject())
    expect(screen.getByText(/Voice guide/i)).toBeInTheDocument()
  })

  it('renders MembersBlock placeholder', () => {
    renderView(makeProject())
    expect(screen.getByText(/Member management coming soon/i)).toBeInTheDocument()
  })

  it('renders BillingBlock', () => {
    renderView(makeProject())
    expect(screen.getByText('Billing')).toBeInTheDocument()
  })

  it('renders MastodonOAuthPanel', () => {
    renderView(makeProject())
    expect(screen.getByText('Mastodon')).toBeInTheDocument()
  })

  it('derives isOwner from org.is_owner', () => {
    renderView(makeProject({ is_owner: false }))
    expect(screen.queryByRole('button', { name: /Add repository/i })).not.toBeInTheDocument()
  })

  it('renders DangerZone toggle for owner below billing', async () => {
    renderView(makeProject())
    const toggle = await screen.findByRole('button', { name: /Danger zone/i })
    expect(toggle).toBeInTheDocument()
    // rows are hidden until expanded
    expect(screen.queryByText(/Disconnect this workspace/i)).not.toBeInTheDocument()
  })

  it('does not render DangerZone rows for non-owner', () => {
    renderView(makeProject({ is_owner: false }))
    expect(screen.queryByText(/Disconnect this workspace/i)).not.toBeInTheDocument()
  })
})
