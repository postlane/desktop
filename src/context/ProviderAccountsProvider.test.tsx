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
import { ProviderAccountsProvider, useProviderAccountsContext } from './ProviderAccountsProvider'

const mockInvoke = vi.mocked(invoke)

const ACCOUNTS = [
  { id: 'row-1', provider: 'github', provider_account_id: '111', label: 'alice', is_primary: true },
]

function Consumer() {
  const ctx = useProviderAccountsContext()
  if (ctx.loading) return <div>loading</div>
  if (ctx.error) return <div>error: {ctx.error}</div>
  return <div data-testid="count">{ctx.accounts.length}</div>
}

beforeEach(() => {
  vi.clearAllMocks()
  capturedListeners = new Map()
})

describe('ProviderAccountsProvider', () => {
  it('provides accounts after successful load', async () => {
    mockInvoke.mockResolvedValueOnce(ACCOUNTS)

    render(<ProviderAccountsProvider><Consumer /></ProviderAccountsProvider>)

    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))
  })

  it('re-fetches accounts when license:activated fires with account_linked=true', async () => {
    mockInvoke
      .mockResolvedValueOnce(ACCOUNTS)
      .mockResolvedValueOnce([...ACCOUNTS, { id: 'row-2', provider: 'github', provider_account_id: '222', label: 'alice-work', is_primary: false }])

    render(<ProviderAccountsProvider><Consumer /></ProviderAccountsProvider>)
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))

    await act(async () => {
      const handlers = capturedListeners.get('license:activated') ?? []
      handlers.forEach((h) => h({ payload: { display_name: 'Alice', account_linked: true } }))
    })

    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'))
  })

  it('does not re-fetch when license:activated fires without account_linked (a normal sign-in)', async () => {
    mockInvoke.mockResolvedValue(ACCOUNTS)

    render(<ProviderAccountsProvider><Consumer /></ProviderAccountsProvider>)
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('1'))

    await act(async () => {
      const handlers = capturedListeners.get('license:activated') ?? []
      handlers.forEach((h) => h({ payload: { display_name: 'Alice', new_link: true } }))
    })

    expect(mockInvoke).toHaveBeenCalledTimes(1)
  })

  it('throws when useProviderAccountsContext is called outside the provider', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    expect(() => render(<Consumer />)).toThrow()
    consoleError.mockRestore()
  })
})
