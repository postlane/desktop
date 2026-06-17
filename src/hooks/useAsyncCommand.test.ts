// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useAsyncCommand } from './useAsyncCommand'

describe('useAsyncCommand — initial state and loading', () => {
  it('starts not loading with no error', () => {
    const { result } = renderHook(() => useAsyncCommand())
    expect(result.current.loading).toBe(false)
    expect(result.current.error).toBeNull()
  })

  it('sets loading true while the operation runs', async () => {
    let resolve!: (v: string) => void
    const pending = new Promise<string>((r) => { resolve = r })
    const { result } = renderHook(() => useAsyncCommand())

    act(() => { void result.current.run(() => pending) })
    expect(result.current.loading).toBe(true)

    await act(async () => { resolve('done') })
    expect(result.current.loading).toBe(false)
  })

  it('returns the resolved value on success', async () => {
    const { result } = renderHook(() => useAsyncCommand())
    let returned: string | null = null
    await act(async () => {
      returned = await result.current.run(() => Promise.resolve('ok'))
    })
    expect(returned).toBe('ok')
  })

  it('error is null after a successful run', async () => {
    const { result } = renderHook(() => useAsyncCommand())
    await act(async () => { await result.current.run(() => Promise.resolve(42)) })
    expect(result.current.error).toBeNull()
  })

  it('loading is false after a failed run', async () => {
    const { result } = renderHook(() => useAsyncCommand())
    await act(async () => {
      await result.current.run(() => Promise.reject(new Error('fail')))
    })
    expect(result.current.loading).toBe(false)
  })
})

describe('useAsyncCommand — error handling', () => {
  it('sets error string when the operation throws an Error', async () => {
    const { result } = renderHook(() => useAsyncCommand())
    await act(async () => {
      await result.current.run(() => Promise.reject(new Error('boom')))
    })
    expect(result.current.error).toBe('boom')
  })

  it('sets error string when the operation rejects with a non-Error', async () => {
    const { result } = renderHook(() => useAsyncCommand())
    await act(async () => {
      await result.current.run(() => Promise.reject('string rejection'))
    })
    expect(result.current.error).toBe('string rejection')
  })

  it('returns null when the operation throws', async () => {
    const { result } = renderHook(() => useAsyncCommand())
    let returned: number | null = -1
    await act(async () => {
      returned = await result.current.run<number>(() => Promise.reject(new Error('fail')))
    })
    expect(returned).toBeNull()
  })

  it('clears previous error on the next run', async () => {
    const { result } = renderHook(() => useAsyncCommand())
    await act(async () => {
      await result.current.run(() => Promise.reject(new Error('first error')))
    })
    expect(result.current.error).toBe('first error')

    await act(async () => { await result.current.run(() => Promise.resolve('fine')) })
    expect(result.current.error).toBeNull()
  })
})
