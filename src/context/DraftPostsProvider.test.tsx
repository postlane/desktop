// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, act } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

let capturedListeners: Map<string, ((_event: unknown) => void)[]> = new Map()

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation((event: string, handler: (_event: unknown) => void) => {
    const existing = capturedListeners.get(event) ?? []
    capturedListeners.set(event, [...existing, handler])
    const unlisten = () => {
      const handlers = capturedListeners.get(event) ?? []
      capturedListeners.set(event, handlers.filter((h) => h !== handler))
    }
    return Promise.resolve(unlisten)
  }),
}))

import { invoke } from '../ipc/invoke'
import { DraftPostsProvider, useDraftPostsContext } from './DraftPostsProvider'
import { DRAFT_DETECTED_EVENT } from '../constants/tauriEvents'

const mockInvoke = vi.mocked(invoke)

const DRAFT = {
  repo_id: 'r1', repo_name: 'Repo', repo_path: '/path',
  post_folder: 'post-001', platforms: ['x'], platform: 'x',
  text: 'Hello', status: 'ready' as const, trigger: null, error: null,
  image_url: null, project_id: 'proj-1', schedule: null,
  platform_results: null, llm_model: null, created_at: null,
}

function Consumer() {
  const { drafts, loading, error } = useDraftPostsContext()
  if (loading) return <div>loading</div>
  if (error) return <div>error: {error}</div>
  return <div data-testid="count">{drafts.length}</div>
}

beforeEach(() => {
  vi.clearAllMocks()
  capturedListeners = new Map()
})

describe('DraftPostsProvider', () => {
  it('provides loading state while fetching', async () => {
    let resolve!: (_v: typeof DRAFT[]) => void
    mockInvoke.mockReturnValueOnce(new Promise((r) => { resolve = r }))
    render(<DraftPostsProvider><Consumer /></DraftPostsProvider>)
    expect(screen.getByText('loading')).toBeInTheDocument()
    await act(async () => { resolve([]) })
  })

  it('provides draft count after successful load', async () => {
    mockInvoke.mockResolvedValueOnce([DRAFT])
    render(<DraftPostsProvider><Consumer /></DraftPostsProvider>)
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))
  })

  it('provides error string when invoke rejects', async () => {
    mockInvoke.mockRejectedValueOnce('network failure')
    render(<DraftPostsProvider><Consumer /></DraftPostsProvider>)
    await waitFor(() => expect(screen.getByText(/error:/)).toBeInTheDocument())
  })

  it('re-fetches when DRAFT_DETECTED_EVENT fires', async () => {
    mockInvoke
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([DRAFT])
    render(<DraftPostsProvider><Consumer /></DraftPostsProvider>)
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('0'))
    await act(async () => {
      const handlers = capturedListeners.get(DRAFT_DETECTED_EVENT) ?? []
      handlers.forEach((h) => h({}))
    })
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))
  })

  it('subscribes to DRAFT_DETECTED_EVENT on mount', async () => {
    mockInvoke.mockResolvedValueOnce([])
    render(<DraftPostsProvider><Consumer /></DraftPostsProvider>)
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('0'))
    const listeners = capturedListeners.get(DRAFT_DETECTED_EVENT) ?? []
    expect(listeners).toHaveLength(1)
  })

  it('throws when useDraftPostsContext is called outside the provider', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    expect(() => render(<Consumer />)).toThrow()
    consoleError.mockRestore()
  })
})
