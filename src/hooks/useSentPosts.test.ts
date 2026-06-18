// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { invoke } from '../ipc/invoke'
import { useSentPosts } from './useSentPosts'
import type { PublishedPost } from '../types'

const mockInvoke = vi.mocked(invoke)

function makePublished(overrides: Partial<PublishedPost> = {}): PublishedPost {
  return {
    repo_id: 'r1', repo_name: 'MyRepo', repo_path: '/path',
    post_folder: 'post-001', platforms: ['x'], platform: 'x',
    status: 'sent', scheduler_ids: {}, platform_urls: {},
    provider: null, sent_at: '2024-01-01T10:00:00Z',
    schedule: null, platform_results: null, llm_model: null, created_at: null,
    project_id: 'proj-1',
    ...overrides,
  }
}

const PUBLISHED = makePublished()

beforeEach(() => { vi.clearAllMocks() })

describe('useSentPosts — fetch', () => {
  it('starts in loading state', () => {
    mockInvoke.mockReturnValueOnce(new Promise(() => {}))
    const { result } = renderHook(() => useSentPosts('proj-1'))
    expect(result.current.loading).toBe(true)
    expect(result.current.posts).toHaveLength(0)
    expect(result.current.error).toBeNull()
  })

  it('populates posts on successful invoke', async () => {
    mockInvoke.mockResolvedValueOnce([PUBLISHED])
    const { result } = renderHook(() => useSentPosts('proj-1'))
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.posts).toEqual([PUBLISHED])
    expect(result.current.error).toBeNull()
  })

  it('sets error when invoke rejects', async () => {
    mockInvoke.mockRejectedValueOnce('network error')
    const { result } = renderHook(() => useSentPosts('proj-1'))
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.error).toBeTruthy()
    expect(result.current.posts).toHaveLength(0)
  })

  it('calls get_org_published with the provided projectId', async () => {
    mockInvoke.mockResolvedValueOnce([])
    renderHook(() => useSentPosts('proj-42'))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled())
    expect(mockInvoke).toHaveBeenCalledWith('get_org_published', { projectId: 'proj-42' })
  })
})

describe('useSentPosts — refresh and reactivity', () => {
  it('projectId change triggers reload with new id', async () => {
    mockInvoke.mockResolvedValue([PUBLISHED])
    const { result, rerender } = renderHook((props: { projectId: string }) => useSentPosts(props.projectId), {
      initialProps: { projectId: 'proj-1' },
    })
    await waitFor(() => expect(result.current.loading).toBe(false))
    rerender({ projectId: 'proj-2' })
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(mockInvoke).toHaveBeenCalledTimes(2)
    expect(mockInvoke).toHaveBeenLastCalledWith('get_org_published', { projectId: 'proj-2' })
  })

  it('refresh() triggers a new invoke call', async () => {
    mockInvoke.mockResolvedValue([PUBLISHED])
    const { result } = renderHook(() => useSentPosts('proj-1'))
    await waitFor(() => expect(result.current.loading).toBe(false))
    await act(async () => { result.current.refresh() })
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(mockInvoke).toHaveBeenCalledTimes(2)
  })

  it('stale success response is discarded when a newer request supersedes it', async () => {
    let resolveFirst!: (value: PublishedPost[]) => void
    const firstPromise = new Promise<PublishedPost[]>((res) => { resolveFirst = res })
    mockInvoke
      .mockReturnValueOnce(firstPromise)
      .mockResolvedValueOnce([PUBLISHED])
    const { result } = renderHook(() => useSentPosts('proj-1'))
    await act(async () => { result.current.refresh() })
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(result.current.loading).toBe(false))
    await act(async () => { resolveFirst([]) })
    expect(result.current.posts).toEqual([PUBLISHED])
  })

  it('stale error response is discarded when a newer request supersedes it', async () => {
    let rejectFirst!: (reason: unknown) => void
    const firstPromise = new Promise<PublishedPost[]>((_, rej) => { rejectFirst = rej })
    mockInvoke
      .mockReturnValueOnce(firstPromise)
      .mockResolvedValueOnce([PUBLISHED])
    const { result } = renderHook(() => useSentPosts('proj-1'))
    await act(async () => { result.current.refresh() })
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(result.current.loading).toBe(false))
    await act(async () => { rejectFirst(new Error('stale error')) })
    expect(result.current.posts).toEqual([PUBLISHED])
    expect(result.current.error).toBeNull()
  })
})

describe('useSentPosts — null/non-array IPC result (HIGH-7)', () => {
  it('treats null response as empty array without crashing', async () => {
    mockInvoke.mockResolvedValueOnce(null)
    const { result } = renderHook(() => useSentPosts('proj-1'))
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.posts).toEqual([])
    expect(result.current.error).toBeNull()
  })

  it('treats non-array response as empty array without crashing', async () => {
    mockInvoke.mockResolvedValueOnce({ unexpected: true })
    const { result } = renderHook(() => useSentPosts('proj-1'))
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.posts).toEqual([])
  })
})
