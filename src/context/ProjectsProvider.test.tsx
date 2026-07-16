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
import { ProjectsProvider, useProjectsContext } from '../context/ProjectsProvider'
import { PROJECTS_CHANGED_EVENT, DEEP_LINK_NEW_URL_EVENT } from '../constants/tauriEvents'

const mockInvoke = vi.mocked(invoke)

const PROJECTS = [
  { id: 'p1', name: 'Postlane', workspace_type: 'organization', tier: 'free', billing_active: true, is_owner: true },
]

function Consumer() {
  const ctx = useProjectsContext()
  if (ctx.loading) return <div>loading</div>
  if (ctx.error) return <div>error: {ctx.error}</div>
  return (
    <div>
      <div data-testid="count">{ctx.projects.length}</div>
      <button onClick={ctx.refresh}>refresh</button>
    </div>
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  capturedListeners = new Map()
})

describe('ProjectsProvider', () => {
  it('provides loading state while fetching', async () => {
    let resolve!: (_v: typeof PROJECTS) => void
    mockInvoke.mockReturnValueOnce(new Promise((r) => { resolve = r }))

    render(<ProjectsProvider><Consumer /></ProjectsProvider>)
    expect(screen.getByText('loading')).toBeInTheDocument()

    await act(async () => { resolve(PROJECTS) })
  })

  it('provides projects after successful load', async () => {
    mockInvoke.mockResolvedValueOnce(PROJECTS)

    render(<ProjectsProvider><Consumer /></ProjectsProvider>)

    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))
  })

  it('provides error string when invoke rejects', async () => {
    mockInvoke.mockRejectedValueOnce('network failure')

    render(<ProjectsProvider><Consumer /></ProjectsProvider>)

    await waitFor(() => expect(screen.getByText(/error:/)).toBeInTheDocument())
  })

  it('re-fetches projects when PROJECTS_CHANGED_EVENT fires', async () => {
    mockInvoke
      .mockResolvedValueOnce(PROJECTS)
      .mockResolvedValueOnce([...PROJECTS, { id: 'p2', name: 'Second', workspace_type: 'personal', tier: 'free', billing_active: true, is_owner: true }])

    render(<ProjectsProvider><Consumer /></ProjectsProvider>)

    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))

    await act(async () => {
      const handlers = capturedListeners.get(PROJECTS_CHANGED_EVENT) ?? []
      handlers.forEach((h) => h({}))
    })

    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'))
  })

  it('throws when useProjectsContext is called outside the provider', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    expect(() => render(<Consumer />)).toThrow()
    consoleError.mockRestore()
  })
})

describe('ProjectsProvider — 24.4.5a billing-complete deep link', () => {
  it('refreshes projects when a billing_complete deep link arrives', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'classify_deep_link') return Promise.resolve({ kind: 'billing_complete', project_id: 'proj-1' })
      return Promise.resolve(PROJECTS)
    })

    render(<ProjectsProvider><Consumer /></ProjectsProvider>)
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))

    const callsBefore = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'list_projects').length

    await act(async () => {
      const handlers = capturedListeners.get(DEEP_LINK_NEW_URL_EVENT) ?? []
      handlers.forEach((h) => h({ payload: ['postlane://billing-complete?project_id=proj-1'] }))
    })

    await waitFor(() => {
      const callsAfter = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'list_projects').length
      expect(callsAfter).toBe(callsBefore + 1)
    })
    expect(mockInvoke).toHaveBeenCalledWith('classify_deep_link', { url: 'postlane://billing-complete?project_id=proj-1' })
  })

  it('does not refresh projects for an unrelated deep link', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'classify_deep_link') return Promise.resolve({ kind: 'activate', project_id: null })
      return Promise.resolve(PROJECTS)
    })

    render(<ProjectsProvider><Consumer /></ProjectsProvider>)
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))

    const callsBefore = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'list_projects').length

    await act(async () => {
      const handlers = capturedListeners.get(DEEP_LINK_NEW_URL_EVENT) ?? []
      handlers.forEach((h) => h({ payload: ['postlane://activate?token=abc'] }))
    })

    // give any (incorrect) refresh a chance to fire before asserting it didn't
    await new Promise((r) => setTimeout(r, 10))
    const callsAfter = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'list_projects').length
    expect(callsAfter).toBe(callsBefore)
  })
})

describe('ProjectsProvider — 24.4.11c workspace_upgraded telemetry', () => {
  it('calls record_billing_complete_upgrade with the project_id from a billing_complete deep link', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'classify_deep_link') return Promise.resolve({ kind: 'billing_complete', project_id: 'proj-1' })
      if (cmd === 'record_billing_complete_upgrade') return Promise.resolve(undefined)
      return Promise.resolve(PROJECTS)
    })

    render(<ProjectsProvider><Consumer /></ProjectsProvider>)
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))

    await act(async () => {
      const handlers = capturedListeners.get(DEEP_LINK_NEW_URL_EVENT) ?? []
      handlers.forEach((h) => h({ payload: ['postlane://billing-complete?project_id=proj-1'] }))
    })

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('record_billing_complete_upgrade', { projectId: 'proj-1' })
    })
  })

  it('does not call record_billing_complete_upgrade for an unrelated deep link', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'classify_deep_link') return Promise.resolve({ kind: 'activate', project_id: null })
      return Promise.resolve(PROJECTS)
    })

    render(<ProjectsProvider><Consumer /></ProjectsProvider>)
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))

    await act(async () => {
      const handlers = capturedListeners.get(DEEP_LINK_NEW_URL_EVENT) ?? []
      handlers.forEach((h) => h({ payload: ['postlane://activate?token=abc'] }))
    })

    await new Promise((r) => setTimeout(r, 10))
    expect(mockInvoke).not.toHaveBeenCalledWith('record_billing_complete_upgrade', expect.anything())
  })

  it('does not fail the deep link handling when record_billing_complete_upgrade rejects', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'classify_deep_link') return Promise.resolve({ kind: 'billing_complete', project_id: 'proj-1' })
      if (cmd === 'record_billing_complete_upgrade') return Promise.reject(new Error('network failure'))
      return Promise.resolve(PROJECTS)
    })

    render(<ProjectsProvider><Consumer /></ProjectsProvider>)
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))

    await act(async () => {
      const handlers = capturedListeners.get(DEEP_LINK_NEW_URL_EVENT) ?? []
      handlers.forEach((h) => h({ payload: ['postlane://billing-complete?project_id=proj-1'] }))
    })

    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))
    consoleError.mockRestore()
  })
})
