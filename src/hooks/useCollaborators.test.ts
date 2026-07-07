// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { invoke } from '../ipc/invoke'
import { useCollaborators } from './useCollaborators'

const mockInvoke = vi.mocked(invoke)

const COLLABORATORS = [
  { user_id: 'u1', role: 'admin', added_at: '2026-01-01T00:00:00Z', display_name: 'Ada', avatar_url: null },
  { user_id: 'u2', role: 'member', added_at: '2026-01-02T00:00:00Z', display_name: 'Bob', avatar_url: null },
]

beforeEach(() => {
  vi.clearAllMocks()
})

describe('useCollaborators', () => {
  it('starts in loading state and fetches on mount', async () => {
    mockInvoke.mockResolvedValueOnce(COLLABORATORS)
    const { result } = renderHook(() => useCollaborators('proj-1'))
    expect(result.current.loading).toBe(true)
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(mockInvoke).toHaveBeenCalledWith('list_project_collaborators', { projectId: 'proj-1' })
    expect(result.current.collaborators).toEqual(COLLABORATORS)
  })

  it('sets error when the list fetch rejects', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('offline'))
    const { result } = renderHook(() => useCollaborators('proj-1'))
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.error).toBe('offline')
    expect(result.current.collaborators).toEqual([])
  })

  it('setRole promotes a collaborator and refreshes the list', async () => {
    mockInvoke.mockResolvedValueOnce(COLLABORATORS)
    const { result } = renderHook(() => useCollaborators('proj-1'))
    await waitFor(() => expect(result.current.loading).toBe(false))

    mockInvoke.mockResolvedValueOnce(undefined)
    mockInvoke.mockResolvedValueOnce(COLLABORATORS)
    await act(async () => { await result.current.setRole('u2', 'admin') })

    expect(mockInvoke).toHaveBeenCalledWith('update_collaborator_role', {
      projectId: 'proj-1', userId: 'u2', role: 'admin',
    })
    expect(mockInvoke).toHaveBeenCalledTimes(3)
  })

  it('setRole records actionError and does not refresh when the update fails', async () => {
    mockInvoke.mockResolvedValueOnce(COLLABORATORS)
    const { result } = renderHook(() => useCollaborators('proj-1'))
    await waitFor(() => expect(result.current.loading).toBe(false))

    mockInvoke.mockRejectedValueOnce(new Error('forbidden'))
    await act(async () => { await result.current.setRole('u2', 'admin') })

    expect(result.current.actionError).toBe('forbidden')
    expect(mockInvoke).toHaveBeenCalledTimes(2)
  })

  it('remove deletes a collaborator and refreshes the list', async () => {
    mockInvoke.mockResolvedValueOnce(COLLABORATORS)
    const { result } = renderHook(() => useCollaborators('proj-1'))
    await waitFor(() => expect(result.current.loading).toBe(false))

    mockInvoke.mockResolvedValueOnce(undefined)
    mockInvoke.mockResolvedValueOnce([COLLABORATORS[0]])
    await act(async () => { await result.current.remove('u2') })

    expect(mockInvoke).toHaveBeenCalledWith('remove_project_collaborator', { projectId: 'proj-1', userId: 'u2' })
    await waitFor(() => expect(result.current.collaborators).toEqual([COLLABORATORS[0]]))
  })

  it('remove records actionError when the delete fails', async () => {
    mockInvoke.mockResolvedValueOnce(COLLABORATORS)
    const { result } = renderHook(() => useCollaborators('proj-1'))
    await waitFor(() => expect(result.current.loading).toBe(false))

    mockInvoke.mockRejectedValueOnce(new Error('not_a_collaborator'))
    await act(async () => { await result.current.remove('u2') })

    expect(result.current.actionError).toBe('not_a_collaborator')
  })
})
