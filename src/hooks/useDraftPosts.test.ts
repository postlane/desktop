// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }))

import { invoke } from '../ipc/invoke'
import { useDraftPosts } from './useDraftPosts'
import type { DraftPost } from '../types'

const mockInvoke = vi.mocked(invoke)

function makeDraft(overrides: Partial<DraftPost> = {}): DraftPost {
  return {
    repo_id: 'r1', repo_name: 'MyRepo', repo_path: '/path',
    post_folder: 'post-001', platforms: ['x'], platform: 'x',
    text: 'Hello', status: 'ready', trigger: null, error: null,
    image_url: null, project_id: 'proj-1', schedule: null,
    platform_results: null, llm_model: null, created_at: null,
    ...overrides,
  }
}

const DRAFT = makeDraft()

beforeEach(() => { vi.clearAllMocks() })

describe('useDraftPosts', () => {
  it('starts in loading state', () => {
    mockInvoke.mockReturnValueOnce(new Promise(() => {}))
    const { result } = renderHook(() => useDraftPosts())
    expect(result.current.loading).toBe(true)
    expect(result.current.drafts).toHaveLength(0)
    expect(result.current.error).toBeNull()
  })

  it('populates drafts on successful invoke', async () => {
    mockInvoke.mockResolvedValueOnce([DRAFT])
    const { result } = renderHook(() => useDraftPosts())
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.drafts).toEqual([DRAFT])
    expect(result.current.error).toBeNull()
  })

  it('sets error when invoke rejects', async () => {
    mockInvoke.mockRejectedValueOnce('fetch failed')
    const { result } = renderHook(() => useDraftPosts())
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.error).toBeTruthy()
    expect(result.current.drafts).toHaveLength(0)
  })

  it('refresh() triggers a new invoke call', async () => {
    mockInvoke.mockResolvedValue([DRAFT])
    const { result } = renderHook(() => useDraftPosts())
    await waitFor(() => expect(result.current.loading).toBe(false))
    await act(async () => { result.current.refresh() })
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(mockInvoke).toHaveBeenCalledTimes(2)
  })

  it('clear() resets state to empty without triggering a new fetch', async () => {
    mockInvoke.mockResolvedValueOnce([DRAFT])
    const { result } = renderHook(() => useDraftPosts())
    await waitFor(() => expect(result.current.loading).toBe(false))
    act(() => { result.current.clear() })
    expect(result.current.drafts).toHaveLength(0)
    expect(result.current.loading).toBe(false)
    expect(result.current.error).toBeNull()
    expect(mockInvoke).toHaveBeenCalledTimes(1)
  })

  it('clear() cancels in-flight fetch — stale data does not overwrite empty state', async () => {
    let resolve!: (_v: DraftPost[]) => void
    mockInvoke.mockReturnValueOnce(new Promise((r) => { resolve = r }))
    const { result } = renderHook(() => useDraftPosts())
    expect(result.current.loading).toBe(true)
    act(() => { result.current.clear() })
    expect(result.current.loading).toBe(false)
    await act(async () => { resolve([DRAFT]) })
    expect(result.current.drafts).toHaveLength(0)
  })

  it('includes drafts with null project_id in the returned array', async () => {
    const draft = makeDraft({ project_id: null })
    mockInvoke.mockResolvedValueOnce([draft])
    const { result } = renderHook(() => useDraftPosts())
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.drafts[0].project_id).toBeNull()
  })
})
