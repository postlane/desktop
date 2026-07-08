// SPDX-License-Identifier: BUSL-1.1
// Tests for checklist 24.4.9 — workspace list with billing-status badges and actions.

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import { MantineProvider } from '@mantine/core'

vi.mock('../context/ProjectsProvider', () => ({ useProjectsContext: vi.fn() }))
vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }))

import { useProjectsContext } from '../context/ProjectsProvider'
import { invoke } from '../ipc/invoke'
import { openUrl } from '@tauri-apps/plugin-opener'
import WorkspaceListSection from './WorkspaceListSection'
import type { Project } from '../types'

const mockCtx = vi.mocked(useProjectsContext)
const mockInvoke = vi.mocked(invoke)
const mockOpenUrl = vi.mocked(openUrl)

function makeProject(overrides: Partial<Project> = {}): Project {
  return {
    id: 'proj-1', name: 'Postlane', workspace_type: 'organization', tier: 'free',
    billing_active: true, is_owner: true, status: 'free_owned', ...overrides,
  }
}

function renderSection() {
  return render(
    <MantineProvider>
      <WorkspaceListSection />
    </MantineProvider>,
  )
}

const mockRefresh = vi.fn()

beforeEach(() => {
  vi.clearAllMocks()
})

describe('WorkspaceListSection — badges', () => {
  it('renders nothing when there are no projects', () => {
    mockCtx.mockReturnValue({ projects: [], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    renderSection()
    expect(screen.queryByText('Postlane')).not.toBeInTheDocument()
  })

  it('shows a Free badge with no action for free_owned', () => {
    mockCtx.mockReturnValue({ projects: [makeProject({ status: 'free_owned' })], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    renderSection()
    expect(screen.getByText('Free')).toBeInTheDocument()
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('shows a $5/month badge with a Pause action for paid_owned', () => {
    mockCtx.mockReturnValue({ projects: [makeProject({ status: 'paid_owned' })], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    renderSection()
    expect(screen.getByText('$5/month')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /pause/i })).toBeInTheDocument()
  })

  it('shows an Add to plan action for paid_required', () => {
    mockCtx.mockReturnValue({ projects: [makeProject({ status: 'paid_required' })], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    renderSection()
    expect(screen.getByRole('button', { name: /add to plan/i })).toBeInTheDocument()
  })

  it('shows an Update billing action for payment_failed', () => {
    mockCtx.mockReturnValue({ projects: [makeProject({ status: 'payment_failed' })], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    renderSection()
    expect(screen.getByRole('button', { name: /update billing/i })).toBeInTheDocument()
  })

  it('shows a badge with no action for the deferred collaborator/inactive/unlicensed states', () => {
    mockCtx.mockReturnValue({
      projects: [
        makeProject({ id: 'a', name: 'A', status: 'collaborator' }),
        makeProject({ id: 'b', name: 'B', status: 'inactive' }),
        makeProject({ id: 'c', name: 'C', status: 'unlicensed' }),
      ],
      loading: false, error: null, refresh: mockRefresh, clear: vi.fn(),
    })
    renderSection()
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })
})

describe('WorkspaceListSection — actions', () => {
  it('Pause calls deactivate_workspace and refreshes on success', async () => {
    mockCtx.mockReturnValue({ projects: [makeProject({ status: 'paid_owned' })], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    mockInvoke.mockResolvedValueOnce(undefined)
    renderSection()

    fireEvent.click(screen.getByRole('button', { name: /pause/i }))

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('deactivate_workspace', { projectId: 'proj-1' }))
    await waitFor(() => expect(mockRefresh).toHaveBeenCalled())
  })

  it('Add to plan calls subscribe_workspace and opens the checkout URL when present', async () => {
    mockCtx.mockReturnValue({ projects: [makeProject({ status: 'paid_required' })], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    mockInvoke.mockResolvedValueOnce('https://checkout.stripe.com/abc')
    renderSection()

    fireEvent.click(screen.getByRole('button', { name: /add to plan/i }))

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('subscribe_workspace', { projectId: 'proj-1' }))
    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalledWith('https://checkout.stripe.com/abc'))
    await waitFor(() => expect(mockRefresh).toHaveBeenCalled())
  })

  it('Add to plan does not open a browser tab when checkout_url is null', async () => {
    mockCtx.mockReturnValue({ projects: [makeProject({ status: 'paid_required' })], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    mockInvoke.mockResolvedValueOnce(null)
    renderSection()

    fireEvent.click(screen.getByRole('button', { name: /add to plan/i }))

    await waitFor(() => expect(mockRefresh).toHaveBeenCalled())
    expect(mockOpenUrl).not.toHaveBeenCalled()
  })

  it('Update billing calls open_billing_portal and opens the returned URL', async () => {
    mockCtx.mockReturnValue({ projects: [makeProject({ status: 'payment_failed' })], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    mockInvoke.mockResolvedValueOnce('https://billing.stripe.com/session/xyz')
    renderSection()

    fireEvent.click(screen.getByRole('button', { name: /update billing/i }))

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('open_billing_portal', { projectId: 'proj-1' }))
    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalledWith('https://billing.stripe.com/session/xyz'))
  })

  it('shows an action error and does not refresh when the action fails', async () => {
    mockCtx.mockReturnValue({ projects: [makeProject({ status: 'paid_owned' })], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    mockInvoke.mockRejectedValueOnce(new Error('forbidden'))
    renderSection()

    fireEvent.click(screen.getByRole('button', { name: /pause/i }))

    await waitFor(() => expect(screen.getByText('forbidden')).toBeInTheDocument())
    expect(mockRefresh).not.toHaveBeenCalled()
  })
})
