// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

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

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockResolvedValue(null)
  mockCtx.mockReturnValue({ projects: [], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
})

// ── Active billing ─────────────────────────────────────────────────────────────

describe('BillingBlock — active billing', () => {
  it('shows tier label', () => {
    render(<BillingBlock project={makeProject({ tier: 'pro' })} isOwner={true} />)
    expect(screen.getByText(/pro/i)).toBeInTheDocument()
  })

  it('no alert banner when billing is active', () => {
    render(<BillingBlock project={makeProject({ billing_active: true })} isOwner={true} />)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })
})

// ── Lapsed billing ─────────────────────────────────────────────────────────────

describe('BillingBlock — lapsed billing', () => {
  it('shows alert banner when billing is inactive', () => {
    render(<BillingBlock project={makeProject({ billing_active: false })} isOwner={true} />)
    expect(screen.getByRole('alert')).toBeInTheDocument()
  })

  it('shows Refresh button in alert banner', () => {
    render(<BillingBlock project={makeProject({ billing_active: false })} isOwner={true} />)
    expect(screen.getByRole('button', { name: /I've updated my billing.*Refresh/i })).toBeInTheDocument()
  })

  it('Refresh calls projectsContext.refresh()', () => {
    render(<BillingBlock project={makeProject({ billing_active: false })} isOwner={true} />)
    fireEvent.click(screen.getByRole('button', { name: /I've updated my billing.*Refresh/i }))
    expect(mockRefresh).toHaveBeenCalled()
  })
})

// ── Billing link ───────────────────────────────────────────────────────────────

describe('BillingBlock — billing link', () => {
  it('owner sees manage billing link', () => {
    render(<BillingBlock project={makeProject()} isOwner={true} />)
    expect(screen.getByRole('button', { name: /Manage billing/i })).toBeInTheDocument()
  })

  it('Manage billing opens postlane.dev/billing via openUrl', async () => {
    render(<BillingBlock project={makeProject()} isOwner={true} />)
    fireEvent.click(screen.getByRole('button', { name: /Manage billing/i }))
    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/billing'))
  })

  it('non-owner does not see Manage billing button', () => {
    render(<BillingBlock project={makeProject()} isOwner={false} />)
    expect(screen.queryByRole('button', { name: /Manage billing/i })).not.toBeInTheDocument()
  })
})
