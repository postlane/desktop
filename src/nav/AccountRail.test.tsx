// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import '@testing-library/jest-dom'
import { MantineProvider } from '@mantine/core'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }))

const mockRefresh = vi.fn()
let mockAccounts: Array<{ id: string; provider: string; provider_account_id: string | null; label: string | null; is_primary: boolean }> = []
let mockActiveAccountId: string | null = null

vi.mock('../context/ProviderAccountsProvider', () => ({
  useProviderAccountsContext: () => ({
    accounts: mockAccounts,
    activeAccountId: mockActiveAccountId,
    loading: false,
    error: null,
    refresh: mockRefresh,
    clear: vi.fn(),
    setActiveAccountId: vi.fn(),
  }),
}))

import { invoke } from '../ipc/invoke'
import { openUrl } from '@tauri-apps/plugin-opener'
import AccountRail from './AccountRail'

const mockInvoke = vi.mocked(invoke)
const mockOpenUrl = vi.mocked(openUrl)

const ONE_ACCOUNT = [
  { id: 'row-1', provider: 'github', provider_account_id: '111', label: 'alice', is_primary: true },
]

const TWO_ACCOUNTS = [
  { id: 'row-1', provider: 'github', provider_account_id: '111', label: 'alice', is_primary: true },
  { id: 'row-2', provider: 'github', provider_account_id: '222', label: 'alice-work', is_primary: false },
]

function renderRail(onSwitch = vi.fn()) {
  return render(
    <MantineProvider>
      <AccountRail onSwitch={onSwitch} />
    </MantineProvider>,
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  mockAccounts = []
  mockActiveAccountId = null
})

describe('AccountRail', () => {
  it('renders nothing for a single-account user', () => {
    mockAccounts = ONE_ACCOUNT
    mockActiveAccountId = 'row-1'
    renderRail()
    expect(screen.queryByRole('navigation', { name: /provider accounts/i })).not.toBeInTheDocument()
  })

  it('renders nothing while there are zero accounts', () => {
    mockAccounts = []
    mockActiveAccountId = null
    renderRail()
    expect(screen.queryByRole('navigation', { name: /provider accounts/i })).not.toBeInTheDocument()
  })

  it('shows one icon per connected account for a 2+ account user', () => {
    mockAccounts = TWO_ACCOUNTS
    mockActiveAccountId = 'row-1'
    renderRail()
    expect(screen.getByRole('button', { name: /^alice-work$/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^alice$/i })).toBeInTheDocument()
  })

  it('calls onSwitch with the account id when an icon is clicked', async () => {
    mockAccounts = TWO_ACCOUNTS
    mockActiveAccountId = 'row-1'
    const onSwitch = vi.fn()
    renderRail(onSwitch)

    await userEvent.click(screen.getByRole('button', { name: /^alice-work$/i }))
    expect(onSwitch).toHaveBeenCalledWith('row-2')
  })

  it('disables the Remove option for the primary account', async () => {
    mockAccounts = TWO_ACCOUNTS
    mockActiveAccountId = 'row-1'
    renderRail()

    await userEvent.click(screen.getByRole('button', { name: /options for alice$/i }))
    expect(await screen.findByRole('menuitem', { name: /remove/i })).toHaveAttribute('data-disabled', 'true')
  })

  it('enables the Remove option for a non-primary account and removes it on click', async () => {
    mockAccounts = TWO_ACCOUNTS
    mockActiveAccountId = 'row-1'
    mockInvoke.mockResolvedValueOnce(undefined)
    renderRail()

    await userEvent.click(screen.getByRole('button', { name: /options for alice-work/i }))
    const removeItem = await screen.findByRole('menuitem', { name: /remove/i })
    expect(removeItem).not.toHaveAttribute('data-disabled', 'true')

    await userEvent.click(removeItem)
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('remove_provider_account', { id: 'row-2' }))
    await waitFor(() => expect(mockRefresh).toHaveBeenCalled())
  })

  it('opens the browser with mode=link_provider_account when "Add provider account" is clicked', async () => {
    mockAccounts = TWO_ACCOUNTS
    mockActiveAccountId = 'row-1'
    mockInvoke.mockResolvedValueOnce(47312)
    renderRail()

    await userEvent.click(screen.getByRole('button', { name: /add provider account/i }))

    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalledWith(
      expect.stringContaining('mode=link_provider_account'),
    ))
    expect(mockOpenUrl.mock.calls[0][0]).toContain('desktop=1')
  })
})
