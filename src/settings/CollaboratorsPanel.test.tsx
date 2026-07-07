// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import { MantineProvider } from '@mantine/core'

vi.mock('../hooks/useCollaborators', () => ({ useCollaborators: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }))

import { useCollaborators } from '../hooks/useCollaborators'
import { confirm } from '@tauri-apps/plugin-dialog'
import CollaboratorsPanel from './CollaboratorsPanel'

const mockUseCollaborators = vi.mocked(useCollaborators)
const mockConfirm = vi.mocked(confirm)

function renderPanel() {
  return render(
    <MantineProvider>
      <CollaboratorsPanel projectId="proj-1" />
    </MantineProvider>,
  )
}

function makeState(overrides: Partial<ReturnType<typeof useCollaborators>> = {}) {
  return {
    collaborators: [],
    loading: false,
    error: null,
    actionError: null,
    refresh: vi.fn(),
    setRole: vi.fn(),
    remove: vi.fn(),
    ...overrides,
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  mockConfirm.mockResolvedValue(true)
})

describe('CollaboratorsPanel — loading and empty states', () => {
  it('shows a loading message while fetching', () => {
    mockUseCollaborators.mockReturnValue(makeState({ loading: true }))
    renderPanel()
    expect(screen.getByText(/loading/i)).toBeInTheDocument()
  })

  it('shows an empty state when there are no collaborators', () => {
    mockUseCollaborators.mockReturnValue(makeState({ collaborators: [] }))
    renderPanel()
    expect(screen.getByText(/no collaborators/i)).toBeInTheDocument()
  })

  it('shows the fetch error when present', () => {
    mockUseCollaborators.mockReturnValue(makeState({ error: 'offline' }))
    renderPanel()
    expect(screen.getByText('offline')).toBeInTheDocument()
  })
})

describe('CollaboratorsPanel — rendering rows', () => {
  it('renders each collaborator with name and role badge', () => {
    mockUseCollaborators.mockReturnValue(makeState({
      collaborators: [
        { user_id: 'u1', role: 'admin', added_at: '2026-01-01T00:00:00Z', display_name: 'Ada Lovelace', avatar_url: null },
        { user_id: 'u2', role: 'member', added_at: '2026-01-02T00:00:00Z', display_name: 'Bob', avatar_url: null },
      ],
    }))
    renderPanel()
    expect(screen.getByText('Ada Lovelace')).toBeInTheDocument()
    expect(screen.getByText('admin')).toBeInTheDocument()
    expect(screen.getByText('Bob')).toBeInTheDocument()
    expect(screen.getByText('member')).toBeInTheDocument()
  })

  it('falls back to user_id when display_name is null', () => {
    mockUseCollaborators.mockReturnValue(makeState({
      collaborators: [{ user_id: 'u1', role: 'member', added_at: '2026-01-01T00:00:00Z', display_name: null, avatar_url: null }],
    }))
    renderPanel()
    expect(screen.getByText('u1')).toBeInTheDocument()
  })
})

describe('CollaboratorsPanel — promote / demote', () => {
  it('shows Promote to admin for a member and calls setRole on click', () => {
    const setRole = vi.fn()
    mockUseCollaborators.mockReturnValue(makeState({
      collaborators: [{ user_id: 'u1', role: 'member', added_at: '2026-01-01T00:00:00Z', display_name: 'Bob', avatar_url: null }],
      setRole,
    }))
    renderPanel()
    fireEvent.click(screen.getByRole('button', { name: /promote to admin/i }))
    expect(setRole).toHaveBeenCalledWith('u1', 'admin')
  })

  it('shows Demote to member for an admin and calls setRole on click', () => {
    const setRole = vi.fn()
    mockUseCollaborators.mockReturnValue(makeState({
      collaborators: [{ user_id: 'u1', role: 'admin', added_at: '2026-01-01T00:00:00Z', display_name: 'Ada', avatar_url: null }],
      setRole,
    }))
    renderPanel()
    fireEvent.click(screen.getByRole('button', { name: /demote to member/i }))
    expect(setRole).toHaveBeenCalledWith('u1', 'member')
  })
})

describe('CollaboratorsPanel — remove', () => {
  it('asks for confirmation before removing', async () => {
    const remove = vi.fn()
    mockUseCollaborators.mockReturnValue(makeState({
      collaborators: [{ user_id: 'u1', role: 'member', added_at: '2026-01-01T00:00:00Z', display_name: 'Bob', avatar_url: null }],
      remove,
    }))
    renderPanel()
    fireEvent.click(screen.getByRole('button', { name: /remove/i }))
    await waitFor(() => expect(mockConfirm).toHaveBeenCalled())
    expect(remove).toHaveBeenCalledWith('u1')
  })

  it('does not remove when the confirm dialog is declined', async () => {
    mockConfirm.mockResolvedValue(false)
    const remove = vi.fn()
    mockUseCollaborators.mockReturnValue(makeState({
      collaborators: [{ user_id: 'u1', role: 'member', added_at: '2026-01-01T00:00:00Z', display_name: 'Bob', avatar_url: null }],
      remove,
    }))
    renderPanel()
    fireEvent.click(screen.getByRole('button', { name: /remove/i }))
    await waitFor(() => expect(mockConfirm).toHaveBeenCalled())
    expect(remove).not.toHaveBeenCalled()
  })

  it('shows actionError when present', () => {
    mockUseCollaborators.mockReturnValue(makeState({
      collaborators: [{ user_id: 'u1', role: 'member', added_at: '2026-01-01T00:00:00Z', display_name: 'Bob', avatar_url: null }],
      actionError: 'forbidden',
    }))
    renderPanel()
    expect(screen.getByText('forbidden')).toBeInTheDocument()
  })
})

describe('CollaboratorsPanel — search and pagination', () => {
  function makeManyCollaborators(count: number) {
    return Array.from({ length: count }, (_, i) => ({
      user_id: `u${i}`,
      role: 'member',
      added_at: '2026-01-01T00:00:00Z',
      display_name: `Person ${i}`,
      avatar_url: null,
    }))
  }

  it('does not show search or pagination controls for 25 or fewer collaborators', () => {
    mockUseCollaborators.mockReturnValue(makeState({ collaborators: makeManyCollaborators(25) }))
    renderPanel()
    expect(screen.queryByPlaceholderText(/search/i)).not.toBeInTheDocument()
  })

  it('shows pagination controls past 25 collaborators', () => {
    mockUseCollaborators.mockReturnValue(makeState({ collaborators: makeManyCollaborators(26) }))
    renderPanel()
    expect(screen.getByPlaceholderText(/search/i)).toBeInTheDocument()
    expect(screen.getByText('Person 0')).toBeInTheDocument()
    expect(screen.queryByText('Person 25')).not.toBeInTheDocument()
  })

  it('filters by search term client-side', () => {
    mockUseCollaborators.mockReturnValue(makeState({ collaborators: makeManyCollaborators(26) }))
    renderPanel()
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: 'Person 25' } })
    expect(screen.getByText('Person 25')).toBeInTheDocument()
    expect(screen.queryByText('Person 0')).not.toBeInTheDocument()
  })
})
