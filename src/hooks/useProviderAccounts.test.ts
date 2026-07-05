// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { invoke } from '../ipc/invoke'
import { useProviderAccounts } from './useProviderAccounts'

const mockInvoke = vi.mocked(invoke)

const ACCOUNTS = [
  { id: 'row-1', provider: 'github', provider_account_id: '111', label: 'alice', is_primary: true },
  { id: 'row-2', provider: 'github', provider_account_id: '222', label: 'alice-work', is_primary: false },
]

beforeEach(() => {
  vi.clearAllMocks()
})

describe('useProviderAccounts', () => {
  it('starts in loading state before the invoke resolves', async () => {
    let resolveInvoke!: (_v: typeof ACCOUNTS) => void
    mockInvoke.mockReturnValueOnce(new Promise((r) => { resolveInvoke = r }))

    const { result } = renderHook(() => useProviderAccounts())

    expect(result.current.loading).toBe(true)
    expect(result.current.accounts).toHaveLength(0)
    expect(result.current.error).toBeNull()

    await act(async () => { resolveInvoke(ACCOUNTS) })
  })

  it('populates accounts on successful invoke', async () => {
    mockInvoke.mockResolvedValueOnce(ACCOUNTS)

    const { result } = renderHook(() => useProviderAccounts())

    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.accounts).toEqual(ACCOUNTS)
    expect(result.current.error).toBeNull()
  })

  it('sets error when invoke rejects', async () => {
    mockInvoke.mockRejectedValueOnce('network failure')

    const { result } = renderHook(() => useProviderAccounts())

    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.error).toBeTruthy()
    expect(result.current.accounts).toHaveLength(0)
  })

  it('defaults activeAccountId to the primary account once loaded', async () => {
    mockInvoke.mockResolvedValueOnce(ACCOUNTS)

    const { result } = renderHook(() => useProviderAccounts())

    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.activeAccountId).toBe('row-1')
  })

  it('falls back to the first account when none is marked primary', async () => {
    mockInvoke.mockResolvedValueOnce([
      { id: 'row-a', provider: 'github', provider_account_id: '1', label: 'a', is_primary: false },
      { id: 'row-b', provider: 'github', provider_account_id: '2', label: 'b', is_primary: false },
    ])

    const { result } = renderHook(() => useProviderAccounts())

    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.activeAccountId).toBe('row-a')
  })

  it('setActiveAccountId updates which account is active', async () => {
    mockInvoke.mockResolvedValueOnce(ACCOUNTS)
    const { result } = renderHook(() => useProviderAccounts())
    await waitFor(() => expect(result.current.loading).toBe(false))

    act(() => { result.current.setActiveAccountId('row-2') })

    expect(result.current.activeAccountId).toBe('row-2')
  })
})

describe('useProviderAccounts — refresh/clear', () => {
  it('refresh() triggers a new invoke call', async () => {
    mockInvoke.mockResolvedValue(ACCOUNTS)
    const { result } = renderHook(() => useProviderAccounts())
    await waitFor(() => expect(result.current.loading).toBe(false))

    await act(async () => { result.current.refresh() })
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(mockInvoke).toHaveBeenCalledTimes(2)
  })

  it('clear() resets state to empty without triggering a new fetch', async () => {
    mockInvoke.mockResolvedValueOnce(ACCOUNTS)
    const { result } = renderHook(() => useProviderAccounts())
    await waitFor(() => expect(result.current.loading).toBe(false))

    act(() => { result.current.clear() })

    expect(result.current.accounts).toHaveLength(0)
    expect(result.current.activeAccountId).toBeNull()
    expect(result.current.loading).toBe(false)
    expect(result.current.error).toBeNull()
    expect(mockInvoke).toHaveBeenCalledTimes(1)
  })
})
