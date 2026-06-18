// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useTabSwitch } from './usePostEditor'

const mockInvoke = vi.fn()
vi.mock('../ipc/invoke', () => ({ invoke: (...args: unknown[]) => mockInvoke(...args) }))

describe('useTabSwitch', () => {
  beforeEach(() => { mockInvoke.mockResolvedValue(undefined) })

  it('returns a function', () => {
    const ref = { current: '' }
    const { result } = renderHook(() =>
      useTabSwitch(false, 'repo', 'folder', 'x', 'hello', [], ref, vi.fn(), vi.fn())
    )
    expect(typeof result.current).toBe('function')
  })

  it('does not call invoke when not dirty', async () => {
    const ref = { current: 'original' }
    const setText = vi.fn()
    const setPlatform = vi.fn()
    const { result } = renderHook(() =>
      useTabSwitch(false, 'repo', 'folder', 'x', 'hello', [], ref, setPlatform, setText)
    )
    await act(async () => { await result.current('bluesky') })
    expect(mockInvoke).not.toHaveBeenCalled()
    expect(setPlatform).toHaveBeenCalledWith('bluesky')
  })

  it('saves draft via invoke when dirty before switching tab', async () => {
    const ref = { current: 'original' }
    const setText = vi.fn()
    const setPlatform = vi.fn()
    const { result } = renderHook(() =>
      useTabSwitch(true, 'my-repo', 'my-folder', 'x', 'edited text', [], ref, setPlatform, setText)
    )
    await act(async () => { await result.current('bluesky') })
    expect(mockInvoke).toHaveBeenCalledWith('save_post_draft', {
      repoPath: 'my-repo',
      postFolder: 'my-folder',
      platform: 'x',
      text: 'edited text',
    })
    expect(setPlatform).toHaveBeenCalledWith('bluesky')
  })

  it('sets text to sibling text when a matching sibling exists', async () => {
    const ref = { current: '' }
    const setText = vi.fn()
    const siblings = [
      { platform: 'bluesky', text: 'bluesky draft' } as never,
    ]
    const { result } = renderHook(() =>
      useTabSwitch(false, 'repo', 'folder', 'x', '', siblings, ref, vi.fn(), setText)
    )
    await act(async () => { await result.current('bluesky') })
    expect(setText).toHaveBeenCalledWith('bluesky draft')
  })
})
