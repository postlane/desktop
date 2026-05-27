// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))

import { invoke } from '../ipc/invoke'
import { openUrl } from '@tauri-apps/plugin-opener'
import SchedulerBlock from './SchedulerBlock'

const mockInvoke = vi.mocked(invoke)
const mockOpenUrl = vi.mocked(openUrl)

// Default: zernio connected, no named accounts
beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_connected_providers') return ['zernio']
    if (cmd === 'get_scheduler_account_names') return {}
    if (cmd === 'save_scheduler_credential') return {}
    return null
  })
})

// ── List / initial render ─────────────────────────────────────────────────────

describe('SchedulerBlock — list', () => {
  it('calls list_connected_providers on mount', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('list_connected_providers', { repoId: 'proj-1' })
    )
  })

  it('renders connected provider label', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('Zernio')).toBeInTheDocument())
  })

  it('shows empty-state message when nothing is connected', async () => {
    mockInvoke.mockResolvedValue([])
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByText(/No scheduler connected/i)).toBeInTheDocument()
    )
  })

  it('shows Upload Post as available when not in connected list', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('Upload Post')).toBeInTheDocument())
  })

  it('shows Buffer as available when not in connected list', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('Buffer')).toBeInTheDocument())
  })

  it('shows Webhook as available when not in connected list', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('Webhook')).toBeInTheDocument())
  })

  it('connected provider shows Change key and Disconnect for owner', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /Change key/i })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: /Disconnect/i })).toBeInTheDocument()
    })
  })

  it('hides API key form when provider is connected', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('Zernio')).toBeInTheDocument())
    expect(screen.queryByLabelText(/API key/i)).not.toBeInTheDocument()
  })

  it('shows Connect button for each available provider for owner', async () => {
    mockInvoke.mockResolvedValue([])
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => {
      const buttons = screen.getAllByRole('button', { name: /^Connect$/i })
      expect(buttons.length).toBeGreaterThanOrEqual(5)
    })
  })
})

// ── Disconnect ────────────────────────────────────────────────────────────────

describe('SchedulerBlock — disconnect', () => {
  it('calls delete_scheduler_credential with provider and workspace repoId', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Disconnect/i }))
    fireEvent.click(screen.getByRole('button', { name: /Disconnect/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('delete_scheduler_credential', {
        provider: 'zernio',
        repoId: 'proj-1',
      })
    )
  })

  it('re-fetches after disconnect', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Disconnect/i }))
    fireEvent.click(screen.getByRole('button', { name: /Disconnect/i }))
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter((c) => c[0] === 'list_connected_providers')
      expect(calls.length).toBeGreaterThanOrEqual(2)
    })
  })

  it('hides Disconnect button for non-owners', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={false} />)
    await waitFor(() => expect(screen.queryByRole('button', { name: /Disconnect/i })).not.toBeInTheDocument())
  })
})

// ── Connect form — interactions ───────────────────────────────────────────────

describe('SchedulerBlock — connect form interactions', () => {
  beforeEach(() => { mockInvoke.mockResolvedValue([]) })

  it('clicking Connect reveals API key input as type="password"', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() =>
      expect(screen.getByLabelText(/API key/i)).toHaveAttribute('type', 'password')
    )
  })

  it('show/hide toggle switches input type', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => screen.getByLabelText(/API key/i))
    fireEvent.click(screen.getByRole('button', { name: /Show/i }))
    expect(screen.getByLabelText(/API key/i)).toHaveAttribute('type', 'text')
    fireEvent.click(screen.getByRole('button', { name: /Hide/i }))
    expect(screen.getByLabelText(/API key/i)).toHaveAttribute('type', 'password')
  })

  it('calls save_scheduler_credential with provider, apiKey, and workspace repoId', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => screen.getByLabelText(/API key/i))
    fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'sk-test-123' } })
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_scheduler_credential', {
        provider: expect.any(String), apiKey: 'sk-test-123', repoId: 'proj-1',
      })
    )
  })

  it('Cancel button hides the API key form', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => screen.getByLabelText(/API key/i))
    fireEvent.click(screen.getByRole('button', { name: /Cancel/i }))
    expect(screen.queryByLabelText(/API key/i)).not.toBeInTheDocument()
  })
})

// ── Connect form — access and errors ──────────────────────────────────────────

describe('SchedulerBlock — connect form access and errors', () => {
  it('shows error alert when save_scheduler_credential fails', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return []
      if (cmd === 'save_scheduler_credential') throw new Error('Bad API key')
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => screen.getByLabelText(/API key/i))
    fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'bad-key' } })
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/ })[0])
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
  })

  it('hides Connect buttons for non-owners', async () => {
    mockInvoke.mockResolvedValue([])
    render(<SchedulerBlock projectId="proj-1" isOwner={false} />)
    await waitFor(() => expect(screen.queryByRole('button', { name: /^Connect$/i })).not.toBeInTheDocument())
  })

  it('shows empty profiles when list_connected_providers fails', async () => {
    mockInvoke.mockRejectedValue(new Error('IPC error'))
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText(/No scheduler connected/i)).toBeInTheDocument())
  })

  it('shows provider label Upload Post correctly', async () => {
    mockInvoke.mockResolvedValue(['upload_post'])
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('Upload Post')).toBeInTheDocument())
  })
})

// ── Change key ────────────────────────────────────────────────────────────────

describe('SchedulerBlock — change key', () => {
  it('shows Change key button for connected provider', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Change key/i })).toBeInTheDocument()
    )
  })

  it('clicking Change key reveals API key form', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Change key/i }))
    fireEvent.click(screen.getByRole('button', { name: /Change key/i }))
    await waitFor(() => expect(screen.getByLabelText(/API key/i)).toBeInTheDocument())
  })

  it('successful change re-fetches profiles', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    try {
      render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
      await waitFor(() => screen.getByRole('button', { name: /Change key/i }))
      fireEvent.click(screen.getByRole('button', { name: /Change key/i }))
      await waitFor(() => screen.getByLabelText(/API key/i))
      fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'new-key-456' } })
      fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
      await waitFor(() => screen.getByRole('status'))
      await act(async () => { vi.advanceTimersByTime(6000) })
      await waitFor(() => {
        const calls = mockInvoke.mock.calls.filter((c) => c[0] === 'list_connected_providers')
        expect(calls.length).toBeGreaterThanOrEqual(2)
      })
    } finally {
      vi.runAllTimers()
      vi.useRealTimers()
    }
  })
})

// ── Sync accounts button ──────────────────────────────────────────────────────

describe('SchedulerBlock — sync accounts button', () => {
  it('shows Sync accounts button when a provider is connected', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Sync accounts/i })).toBeInTheDocument()
    )
  })

  it('does not show Sync accounts button when no provider is connected', async () => {
    mockInvoke.mockResolvedValue([])
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText(/No scheduler connected/i)).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Sync accounts/i })).not.toBeInTheDocument()
  })
})

describe('SchedulerBlock — sync accounts actions', () => {
  it('calls refresh_scheduler_accounts with repoId on click', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return ['zernio']
      if (cmd === 'get_scheduler_account_names') return {}
      if (cmd === 'refresh_scheduler_accounts') return { providers_synced: ['zernio'], errors: [] }
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Sync accounts/i }))
    fireEvent.click(screen.getByRole('button', { name: /Sync accounts/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('refresh_scheduler_accounts', { repoId: 'proj-1' })
    )
  })

  it('shows success message after sync', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return ['zernio']
      if (cmd === 'get_scheduler_account_names') return {}
      if (cmd === 'refresh_scheduler_accounts') return { providers_synced: ['zernio'], errors: [] }
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Sync accounts/i }))
    fireEvent.click(screen.getByRole('button', { name: /Sync accounts/i }))
    await waitFor(() => expect(screen.getByText(/Synced/i)).toBeInTheDocument())
  })

  it('shows error message when sync returns errors', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return ['zernio']
      if (cmd === 'get_scheduler_account_names') return {}
      if (cmd === 'refresh_scheduler_accounts') return { providers_synced: [], errors: ['zernio: 401 Unauthorized'] }
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Sync accounts/i }))
    fireEvent.click(screen.getByRole('button', { name: /Sync accounts/i }))
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
  })

  it('shows error alert when refresh_scheduler_accounts throws', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return ['zernio']
      if (cmd === 'get_scheduler_account_names') return {}
      if (cmd === 'refresh_scheduler_accounts') throw new Error('IPC error')
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Sync accounts/i }))
    fireEvent.click(screen.getByRole('button', { name: /Sync accounts/i }))
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
  })
})

// ── Account names display ─────────────────────────────────────────────────────

describe('SchedulerBlock — account names', () => {
  it('shows account name tags when get_scheduler_account_names returns data', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return ['zernio']
      if (cmd === 'get_scheduler_account_names') return { bluesky: '@rng_dev', x: '@postlane' }
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('@rng_dev')).toBeInTheDocument())
    expect(screen.getByText('@postlane')).toBeInTheDocument()
  })

  it('shows nothing extra when get_scheduler_account_names returns empty', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return ['zernio']
      if (cmd === 'get_scheduler_account_names') return {}
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('Zernio')).toBeInTheDocument())
    expect(screen.queryByText(/@/)).not.toBeInTheDocument()
  })
})

// ── Provider website links ────────────────────────────────────────────────────

describe('SchedulerBlock — provider links', () => {
  it('shows link icon for connected Zernio row', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Open Zernio website/i })).toBeInTheDocument()
    )
  })

  it('clicking Zernio link icon opens zernio.io', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Open Zernio website/i }))
    fireEvent.click(screen.getByRole('button', { name: /Open Zernio website/i }))
    expect(mockOpenUrl).toHaveBeenCalledWith('https://zernio.io')
  })

  it('shows link icon for available Upload Post row', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Open Upload Post website/i })).toBeInTheDocument()
    )
  })

  it('does not show link icon for Webhook row (no URL)', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByText('Webhook'))
    expect(screen.queryByRole('button', { name: /Open Webhook website/i })).not.toBeInTheDocument()
  })
})

// ── Upload Post — username field ──────────────────────────────────────────────

describe('SchedulerBlock — Upload Post username field', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return []
      if (cmd === 'get_scheduler_account_names') return {}
      if (cmd === 'save_scheduler_credential') return { upload_post: 'postlane', bluesky: 'postlane.bsky.social' }
      return null
    })
  })

  it('shows username input when upload_post connect form is opened', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[1])
    await waitFor(() => expect(screen.getByLabelText(/Upload Post username/i)).toBeInTheDocument())
  })

  it('does not show username input for other providers', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    // First button is zernio
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => screen.getByLabelText(/API key/i))
    expect(screen.queryByLabelText(/Upload Post username/i)).not.toBeInTheDocument()
  })

  it('Connect button is disabled when upload_post username is empty', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[1])
    await waitFor(() => screen.getByLabelText(/API key/i))
    fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'sk-test-123' } })
    // username left empty — ConnectForm's Connect is index 1 (upload_post is 2nd in ALL_PROVIDERS)
    expect(screen.getAllByRole('button', { name: /^Connect$/i })[1]).toBeDisabled()
  })

  it('calls save_scheduler_credential with username for upload_post', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[1])
    await waitFor(() => screen.getByLabelText(/API key/i))
    fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'sk-test-123' } })
    fireEvent.change(screen.getByLabelText(/Upload Post username/i), { target: { value: 'postlane' } })
    // ConnectForm's Connect button is index 1 (upload_post row replaces its row-level button with the form)
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[1])
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_scheduler_credential', {
        provider: 'upload_post',
        apiKey: 'sk-test-123',
        repoId: 'proj-1',
        username: 'postlane',
      })
    )
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent(/user account postlane/i))
  })
})


// ── Connect form — success message ────────────────────────────────────────────

describe('SchedulerBlock — connect form success message', () => {
  beforeEach(() => { vi.useFakeTimers({ shouldAdvanceTime: true }) })
  afterEach(() => { vi.runAllTimers(); vi.useRealTimers() })

  it('shows Connected status after successful save', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return []
      if (cmd === 'get_scheduler_account_names') return {}
      if (cmd === 'save_scheduler_credential') return {}
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => screen.getByLabelText(/API key/i))
    fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'valid-key' } })
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent(/connected/i))
  })

  it('includes org name in success message when account names are returned', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return []
      if (cmd === 'get_scheduler_account_names') return {}
      if (cmd === 'save_scheduler_credential') return { bluesky: 'PostLane Org', x: 'PostLane Org' }
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => screen.getByLabelText(/API key/i))
    fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'valid-key' } })
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent(/PostLane Org/i))
  })
})
