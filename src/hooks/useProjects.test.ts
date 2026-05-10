// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }))

import { invoke } from '../ipc/invoke'
import { useProjects } from './useProjects'

const mockInvoke = vi.mocked(invoke)

const PROJECTS = [
  { id: 'p1', name: 'Postlane', workspace_type: 'organization', tier: 'free', billing_active: true, is_owner: true },
]

beforeEach(() => {
  vi.clearAllMocks()
})

describe('useProjects', () => {
  it('starts in loading state before the invoke resolves', async () => {
    let resolveInvoke!: (_v: typeof PROJECTS) => void
    mockInvoke.mockReturnValueOnce(new Promise((r) => { resolveInvoke = r }))

    const { result } = renderHook(() => useProjects())

    expect(result.current.loading).toBe(true)
    expect(result.current.projects).toHaveLength(0)
    expect(result.current.error).toBeNull()

    await act(async () => { resolveInvoke(PROJECTS) })
  })

  it('populates projects on successful invoke', async () => {
    mockInvoke.mockResolvedValueOnce(PROJECTS)

    const { result } = renderHook(() => useProjects())

    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.projects).toEqual(PROJECTS)
    expect(result.current.error).toBeNull()
  })

  it('sets error when invoke rejects', async () => {
    mockInvoke.mockRejectedValueOnce('network failure')

    const { result } = renderHook(() => useProjects())

    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.error).toBeTruthy()
    expect(result.current.projects).toHaveLength(0)
  })

  it('refresh() triggers a new invoke call', async () => {
    mockInvoke.mockResolvedValue(PROJECTS)
    const { result } = renderHook(() => useProjects())
    await waitFor(() => expect(result.current.loading).toBe(false))

    await act(async () => { result.current.refresh() })
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(mockInvoke).toHaveBeenCalledTimes(2)
  })

  it('clear() resets state to empty without triggering a new fetch', async () => {
    mockInvoke.mockResolvedValueOnce(PROJECTS)
    const { result } = renderHook(() => useProjects())
    await waitFor(() => expect(result.current.loading).toBe(false))

    act(() => { result.current.clear() })

    expect(result.current.projects).toHaveLength(0)
    expect(result.current.loading).toBe(false)
    expect(result.current.error).toBeNull()
    expect(mockInvoke).toHaveBeenCalledTimes(1)
  })

  it('clear() cancels an in-flight invoke so stale data does not overwrite empty state', async () => {
    let resolveInvoke!: (_v: typeof PROJECTS) => void
    mockInvoke.mockReturnValueOnce(new Promise((r) => { resolveInvoke = r }))

    const { result } = renderHook(() => useProjects())
    expect(result.current.loading).toBe(true)

    act(() => { result.current.clear() })
    expect(result.current.loading).toBe(false)
    expect(result.current.projects).toHaveLength(0)

    await act(async () => { resolveInvoke(PROJECTS) })

    expect(result.current.projects).toHaveLength(0)
  })
})
