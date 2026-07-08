// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import '@testing-library/jest-dom'
import { MantineProvider } from '@mantine/core'

vi.mock('../context/ProjectsProvider', () => ({ useProjectsContext: vi.fn() }))
vi.mock('./CollaboratorsPanel', () => ({ default: ({ projectId }: { projectId: string }) => <div data-testid={`panel-${projectId}`} /> }))

import { useProjectsContext } from '../context/ProjectsProvider'
import ManageCollaboratorsSection from './ManageCollaboratorsSection'
import type { Project } from '../types'

const mockCtx = vi.mocked(useProjectsContext)

function makeProject(overrides: Partial<Project> = {}): Project {
  return { id: 'proj-1', name: 'Postlane', workspace_type: 'organization', tier: 'free', billing_active: true, is_owner: true, ...overrides }
}

function renderSection() {
  return render(
    <MantineProvider>
      <ManageCollaboratorsSection />
    </MantineProvider>,
  )
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe('ManageCollaboratorsSection', () => {
  it('renders nothing when the user owns no workspaces', () => {
    mockCtx.mockReturnValue({ projects: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
    renderSection()
    expect(screen.queryByText('Workspaces')).not.toBeInTheDocument()
  })

  it('lists only owned workspaces, excluding collaborator-only ones', () => {
    mockCtx.mockReturnValue({
      projects: [
        makeProject({ id: 'owned-1', name: 'Owned Workspace', is_owner: true }),
        makeProject({ id: 'collab-1', name: 'Collaborator Workspace', is_owner: false }),
      ],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderSection()
    expect(screen.getByText('Owned Workspace')).toBeInTheDocument()
    expect(screen.queryByText('Collaborator Workspace')).not.toBeInTheDocument()
  })

  it('expanding a workspace renders its CollaboratorsPanel', () => {
    mockCtx.mockReturnValue({
      projects: [makeProject({ id: 'proj-1', name: 'My Workspace' })],
      loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    })
    renderSection()
    fireEvent.click(screen.getByText('My Workspace'))
    expect(screen.getByTestId('panel-proj-1')).toBeInTheDocument()
  })
})
