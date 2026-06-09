// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('../context/ProjectsProvider', () => ({ useProjectsContext: vi.fn() }))
vi.mock('../context/DraftPostsProvider', () => ({ useDraftPostsContext: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))

import { invoke } from '../ipc/invoke'
import { useProjectsContext } from '../context/ProjectsProvider'
import { useDraftPostsContext } from '../context/DraftPostsProvider'
import { openUrl } from '@tauri-apps/plugin-opener'
import AccountSettingsView from './AccountSettingsView'

const mockInvoke = vi.mocked(invoke)
const mockProjectsCtx = vi.mocked(useProjectsContext)
const mockDraftCtx = vi.mocked(useDraftPostsContext)
const mockOpenUrl = vi.mocked(openUrl)
const mockClearProjects = vi.fn()
const mockClearDrafts = vi.fn()

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'get_license_display_name') return 'alice@example.com'
    if (cmd === 'get_license_email') return 'alice@example.com'
    if (cmd === 'get_deletion_incomplete') return false
    if (cmd === 'sign_out') return undefined
    return null
  })
  mockProjectsCtx.mockReturnValue({ projects: [], loading: false, error: null, refresh: vi.fn(), clear: mockClearProjects })
  mockDraftCtx.mockReturnValue({ drafts: [], loading: false, error: null, refresh: vi.fn(), clear: mockClearDrafts })
})

// ── Display name ───────────────────────────────────────────────────────────────

describe('AccountSettingsView — display name', () => {
  it('calls get_license_display_name on mount', async () => {
    render(<AccountSettingsView onSignedOut={vi.fn()} />)
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_license_display_name'))
  })

  it('shows the display name', async () => {
    render(<AccountSettingsView onSignedOut={vi.fn()} />)
    await waitFor(() => expect(screen.getByText('alice@example.com')).toBeInTheDocument())
  })

  it('shows fallback when display name is null', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_license_display_name') return null
      return undefined
    })
    render(<AccountSettingsView onSignedOut={vi.fn()} />)
    await waitFor(() => expect(screen.getByText('Signed in')).toBeInTheDocument())
  })
})

// ── Sign out ───────────────────────────────────────────────────────────────────

describe('AccountSettingsView — sign out', () => {
  it('calls sign_out invoke when Sign out is clicked', async () => {
    render(<AccountSettingsView onSignedOut={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /Sign out/i }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('sign_out'))
  })

  it('calls projectsContext.clear() on sign out', async () => {
    render(<AccountSettingsView onSignedOut={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /Sign out/i }))
    await waitFor(() => expect(mockClearProjects).toHaveBeenCalled())
  })

  it('calls draftPostsContext.clear() on sign out', async () => {
    render(<AccountSettingsView onSignedOut={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /Sign out/i }))
    await waitFor(() => expect(mockClearDrafts).toHaveBeenCalled())
  })

  it('calls onSignedOut callback after sign out', async () => {
    const onSignedOut = vi.fn()
    render(<AccountSettingsView onSignedOut={onSignedOut} />)
    fireEvent.click(screen.getByRole('button', { name: /Sign out/i }))
    await waitFor(() => expect(onSignedOut).toHaveBeenCalled())
  })

  it('calls onSignedOut even when sign_out command errors (keyring entry already gone)', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_license_display_name') return 'alice@example.com'
      if (cmd === 'get_license_email') return 'alice@example.com'
      if (cmd === 'get_deletion_incomplete') return false
      if (cmd === 'sign_out') throw new Error('Failed to sign out: No matching entry found in secure storage')
      return null
    })
    const onSignedOut = vi.fn()
    render(<AccountSettingsView onSignedOut={onSignedOut} />)
    fireEvent.click(screen.getByRole('button', { name: /Sign out/i }))
    await waitFor(() => expect(onSignedOut).toHaveBeenCalled())
  })
})

// ── Account danger zone visibility ────────────────────────────────────────────

describe('AccountSettingsView — account danger zone', () => {
  it('renders the danger zone when email is null but display name is available', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_license_display_name') return 'throwaway-user'
      if (cmd === 'get_license_email') return null
      if (cmd === 'get_deletion_incomplete') return false
      return undefined
    })
    render(<AccountSettingsView onSignedOut={vi.fn()} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Danger Zone/i })).toBeInTheDocument()
    )
  })
})

// ── Account link ───────────────────────────────────────────────────────────────

describe('AccountSettingsView — account link', () => {
  it('opens postlane.dev/account via openUrl', async () => {
    render(<AccountSettingsView onSignedOut={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /Manage account/i }))
    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/account'))
  })

  it('does not render a Delete Account button', () => {
    render(<AccountSettingsView onSignedOut={vi.fn()} />)
    expect(screen.queryByRole('button', { name: /Delete account/i })).not.toBeInTheDocument()
  })
})
