// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import { MantineProvider } from '@mantine/core'

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))
vi.mock('../context/ProjectsProvider', () => ({ useProjectsContext: vi.fn() }))
vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { openUrl } from '@tauri-apps/plugin-opener'
import { useProjectsContext } from '../context/ProjectsProvider'
import { invoke } from '../ipc/invoke'
import BillingBlock from './BillingBlock'
import type { Project } from '../types'

const mockOpenUrl = vi.mocked(openUrl)
const mockCtx = vi.mocked(useProjectsContext)
const mockInvoke = vi.mocked(invoke)
const mockRefresh = vi.fn()

function makeProject(overrides: Partial<Project> = {}): Project {
  return { id: 'proj-1', name: 'Postlane', workspace_type: 'organization', tier: 'free', billing_active: true, is_owner: true, ...overrides }
}

// BillingBlock renders WithdrawFromContractButton (checklist 24.4.13), a
// Mantine component, alongside its own still-Bulma markup -- every render
// needs a MantineProvider now, matching this repo's established
// convention for components with any Mantine child (WorkspaceListSection.
// test.tsx).
function renderBlock(project: Project, isOwner: boolean) {
  return render(
    <MantineProvider>
      <BillingBlock project={project} isOwner={isOwner} />
    </MantineProvider>,
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockResolvedValue(null)
  mockCtx.mockReturnValue({ projects: [], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
})

// ── Active billing ─────────────────────────────────────────────────────────────

describe('BillingBlock — active billing', () => {
  it('shows tier label', () => {
    renderBlock(makeProject({ tier: 'pro' }), true)
    expect(screen.getByText(/pro/i)).toBeInTheDocument()
  })

  it('no alert banner when billing is active', () => {
    renderBlock(makeProject({ billing_active: true }), true)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })
})

// ── Lapsed billing ─────────────────────────────────────────────────────────────

describe('BillingBlock — lapsed billing', () => {
  it('shows alert banner when billing is inactive', () => {
    renderBlock(makeProject({ billing_active: false }), true)
    expect(screen.getByRole('alert')).toBeInTheDocument()
  })

  it('shows Refresh button in alert banner', () => {
    renderBlock(makeProject({ billing_active: false }), true)
    expect(screen.getByRole('button', { name: /I've updated my billing.*Refresh/i })).toBeInTheDocument()
  })

  it('Refresh calls projectsContext.refresh()', () => {
    renderBlock(makeProject({ billing_active: false }), true)
    fireEvent.click(screen.getByRole('button', { name: /I've updated my billing.*Refresh/i }))
    expect(mockRefresh).toHaveBeenCalled()
  })
})

// ── Billing link ───────────────────────────────────────────────────────────────

describe('BillingBlock — billing link', () => {
  it('owner sees manage billing link', () => {
    renderBlock(makeProject(), true)
    expect(screen.getByRole('button', { name: /Manage billing/i })).toBeInTheDocument()
  })

  it('Manage billing opens postlane.dev/billing via openUrl', async () => {
    renderBlock(makeProject(), true)
    fireEvent.click(screen.getByRole('button', { name: /Manage billing/i }))
    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/billing'))
  })

  it('non-owner does not see Manage billing button', () => {
    renderBlock(makeProject(), false)
    expect(screen.queryByRole('button', { name: /Manage billing/i })).not.toBeInTheDocument()
  })
})

// ── Withdrawal button (checklist 24.4.13) ─────────────────────────────────────

describe('BillingBlock — withdrawal button', () => {
  it('owner sees the Withdraw from contract button alongside Manage billing', () => {
    renderBlock(makeProject(), true)
    expect(screen.getByRole('button', { name: 'Withdraw from contract' })).toBeInTheDocument()
  })

  it('non-owner does not see the Withdraw from contract button, same gating as Manage billing', () => {
    renderBlock(makeProject(), false)
    expect(screen.queryByRole('button', { name: 'Withdraw from contract' })).not.toBeInTheDocument()
  })
})
