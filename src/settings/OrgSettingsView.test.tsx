// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import '@testing-library/jest-dom'

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

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_scheduler_profiles') return []
    if (cmd === 'get_project_voice_guide') return ''
    return null
  })
  mockUseProjectRepos.mockReturnValue({ repos: [], loadError: null, refresh: vi.fn() })
  mockCtx.mockReturnValue({ projects: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
})

describe('OrgSettingsView', () => {
  it('renders RepositoriesBlock', () => {
    render(<OrgSettingsView org={makeProject()} />)
    expect(screen.getByText(/No repositories connected/i)).toBeInTheDocument()
  })

  it('renders SchedulerBlock', () => {
    render(<OrgSettingsView org={makeProject()} />)
    expect(screen.getByText('Scheduler')).toBeInTheDocument()
  })

  it('renders VoiceGuideBlock', () => {
    render(<OrgSettingsView org={makeProject()} />)
    expect(screen.getByText(/Voice guide/i)).toBeInTheDocument()
  })

  it('renders MembersBlock placeholder', () => {
    render(<OrgSettingsView org={makeProject()} />)
    expect(screen.getByText(/Member management coming soon/i)).toBeInTheDocument()
  })

  it('renders BillingBlock', () => {
    render(<OrgSettingsView org={makeProject()} />)
    expect(screen.getByText('Billing')).toBeInTheDocument()
  })

  it('derives isOwner from org.is_owner', () => {
    render(<OrgSettingsView org={makeProject({ is_owner: false })} />)
    expect(screen.queryByRole('button', { name: /Add repository/i })).not.toBeInTheDocument()
  })
})
