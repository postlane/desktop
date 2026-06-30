// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }))

import { invoke } from '../ipc/invoke'
import { useAsyncList } from './useAsyncList'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => { vi.clearAllMocks() })

describe('useAsyncList — state management', () => {
  it('starts in loading state with empty data', () => {
    mockInvoke.mockReturnValueOnce(new Promise(() => {}))
    const { result } = renderHook(() => useAsyncList<string>('test_command'))
    expect(result.current.loading).toBe(true)
    expect(result.current.data).toHaveLength(0)
    expect(result.current.error).toBeNull()
  })

  it('populates data on successful invoke', async () => {
    mockInvoke.mockResolvedValueOnce(['a', 'b'])
    const { result } = renderHook(() => useAsyncList<string>('test_command'))
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.data).toEqual(['a', 'b'])
    expect(result.current.error).toBeNull()
  })

  it('sets error when invoke rejects', async () => {
    mockInvoke.mockRejectedValueOnce('fetch failed')
    const { result } = renderHook(() => useAsyncList<string>('test_command'))
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.error).toBeTruthy()
    expect(result.current.data).toHaveLength(0)
  })

  it('refresh() triggers a new invoke call', async () => {
    mockInvoke.mockResolvedValue(['x'])
    const { result } = renderHook(() => useAsyncList<string>('test_command'))
    await waitFor(() => expect(result.current.loading).toBe(false))
    await act(async () => { result.current.refresh() })
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(mockInvoke).toHaveBeenCalledTimes(2)
  })

  it('clear() resets to empty without triggering a new fetch', async () => {
    mockInvoke.mockResolvedValueOnce(['x'])
    const { result } = renderHook(() => useAsyncList<string>('test_command'))
    await waitFor(() => expect(result.current.loading).toBe(false))
    act(() => { result.current.clear() })
    expect(result.current.data).toHaveLength(0)
    expect(result.current.loading).toBe(false)
    expect(result.current.error).toBeNull()
    expect(mockInvoke).toHaveBeenCalledTimes(1)
  })
})

describe('useAsyncList — args handling', () => {
  it('clear() cancels in-flight fetch — stale data does not overwrite empty state', async () => {
    let resolve!: (_v: string[]) => void
    mockInvoke.mockReturnValueOnce(new Promise((r) => { resolve = r }))
    const { result } = renderHook(() => useAsyncList<string>('test_command'))
    expect(result.current.loading).toBe(true)
    act(() => { result.current.clear() })
    expect(result.current.loading).toBe(false)
    await act(async () => { resolve(['stale']) })
    expect(result.current.data).toHaveLength(0)
  })

  it('passes args to invoke', async () => {
    mockInvoke.mockResolvedValueOnce([])
    renderHook(() => useAsyncList<string>('test_command', { projectId: 'proj-1' }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('test_command', { projectId: 'proj-1' }))
  })

  it('re-loads when args content changes', async () => {
    mockInvoke.mockResolvedValue([])
    const { result, rerender } = renderHook(
      ({ id }) => useAsyncList<string>('test_command', { projectId: id }),
      { initialProps: { id: 'proj-1' } },
    )
    await waitFor(() => expect(result.current.loading).toBe(false))
    rerender({ id: 'proj-2' })
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(2))
  })
})
