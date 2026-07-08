// SPDX-License-Identifier: BUSL-1.1
// Tests for checklist 24.4.9 — Connected accounts section (Disconnect, Add another account).

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import { MantineProvider } from '@mantine/core'

vi.mock('../context/ProviderAccountsProvider', () => ({ useProviderAccountsContext: vi.fn() }))
vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('../auth/providerAccountLinking', () => ({ startLinkProviderAccountFlow: vi.fn() }))

import { useProviderAccountsContext } from '../context/ProviderAccountsProvider'
import { invoke } from '../ipc/invoke'
import { startLinkProviderAccountFlow } from '../auth/providerAccountLinking'
import ConnectedAccountsSection from './ConnectedAccountsSection'
import type { ProviderAccountSummary } from '../hooks/useProviderAccounts'

const mockCtx = vi.mocked(useProviderAccountsContext)
const mockInvoke = vi.mocked(invoke)
const mockStartLink = vi.mocked(startLinkProviderAccountFlow)

function makeAccount(overrides: Partial<ProviderAccountSummary> = {}): ProviderAccountSummary {
  return { id: 'acc-1', provider: 'github', provider_account_id: '123', label: 'octocat', is_primary: true, ...overrides }
}

const mockRefresh = vi.fn()

function renderSection() {
  return render(
    <MantineProvider>
      <ConnectedAccountsSection />
    </MantineProvider>,
  )
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe('ConnectedAccountsSection', () => {
  it('renders one row per connected provider account', () => {
    mockCtx.mockReturnValue({
      accounts: [makeAccount({ id: 'a', provider: 'github', label: 'octocat' }), makeAccount({ id: 'b', provider: 'gitlab', label: 'gl-user', is_primary: false })],
      activeAccountId: 'a', setActiveAccountId: vi.fn(), loading: false, error: null, refresh: mockRefresh, clear: vi.fn(),
    })
    renderSection()
    expect(screen.getByText('octocat')).toBeInTheDocument()
    expect(screen.getByText('gl-user')).toBeInTheDocument()
  })

  it('falls back to the provider name when label is null', () => {
    mockCtx.mockReturnValue({
      accounts: [makeAccount({ label: null, provider: 'github' })],
      activeAccountId: 'acc-1', setActiveAccountId: vi.fn(), loading: false, error: null, refresh: mockRefresh, clear: vi.fn(),
    })
    renderSection()
    expect(screen.getByText('github')).toBeInTheDocument()
  })

  it('disables Disconnect with a tooltip when only one provider is connected', () => {
    mockCtx.mockReturnValue({
      accounts: [makeAccount()],
      activeAccountId: 'acc-1', setActiveAccountId: vi.fn(), loading: false, error: null, refresh: mockRefresh, clear: vi.fn(),
    })
    renderSection()
    const disconnectButton = screen.getByRole('button', { name: /disconnect/i });
    expect(disconnectButton).toBeDisabled()
  })

  it('enables Disconnect when more than one provider is connected', async () => {
    mockCtx.mockReturnValue({
      accounts: [makeAccount({ id: 'a' }), makeAccount({ id: 'b', is_primary: false })],
      activeAccountId: 'a', setActiveAccountId: vi.fn(), loading: false, error: null, refresh: mockRefresh, clear: vi.fn(),
    })
    mockInvoke.mockResolvedValueOnce(undefined)
    renderSection()

    const disconnectButtons = screen.getAllByRole('button', { name: /disconnect/i });
    expect(disconnectButtons[0]).not.toBeDisabled()
    fireEvent.click(disconnectButtons[0])

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('remove_provider_account', { id: 'a' }))
    await waitFor(() => expect(mockRefresh).toHaveBeenCalled())
  })

  it('shows an Add another account button that starts the link-provider-account flow', () => {
    mockCtx.mockReturnValue({
      accounts: [makeAccount()],
      activeAccountId: 'acc-1', setActiveAccountId: vi.fn(), loading: false, error: null, refresh: mockRefresh, clear: vi.fn(),
    })
    renderSection()

    fireEvent.click(screen.getByRole('button', { name: /add another account/i }))
    expect(mockStartLink).toHaveBeenCalled()
  })
})
