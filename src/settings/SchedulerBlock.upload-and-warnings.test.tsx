// SPDX-License-Identifier: BUSL-1.1
// Upload Post username field, connect-form success messages, and sync-warning tests.
// Split from SchedulerBlock.test.tsx to keep both files under the 400-line limit.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))

import { invoke } from '../ipc/invoke'
import SchedulerBlock from './SchedulerBlock'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_connected_providers') return ['zernio']
    if (cmd === 'get_scheduler_account_names') return {}
    if (cmd === 'save_scheduler_credential') return { account_names: {}, sync_warning: null }
    return null
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
    expect(screen.getAllByRole('button', { name: /^Connect$/i })[1]).toBeDisabled()
  })

  it('calls save_scheduler_credential with username for upload_post', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return []
      if (cmd === 'get_scheduler_account_names') return {}
      if (cmd === 'save_scheduler_credential') return { account_names: { upload_post: 'postlane' }, sync_warning: null }
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[1])
    await waitFor(() => screen.getByLabelText(/API key/i))
    fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'sk-test-123' } })
    fireEvent.change(screen.getByLabelText(/Upload Post username/i), { target: { value: 'postlane' } })
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[1])
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_scheduler_credential', {
        provider: 'upload_post', apiKey: 'sk-test-123', repoId: 'proj-1', username: 'postlane',
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
      if (cmd === 'save_scheduler_credential') return { account_names: {}, sync_warning: null }
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
      if (cmd === 'save_scheduler_credential') return { account_names: { bluesky: 'PostLane Org', x: 'PostLane Org' }, sync_warning: null }
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

// ── Sync warnings surfaced to user (MEDIUM-2) ─────────────────────────────────

describe('SchedulerBlock — sync warnings', () => {
  it('shows a warning notification when save_scheduler_credential returns sync_warning', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return []
      if (cmd === 'get_scheduler_account_names') return {}
      if (cmd === 'save_scheduler_credential') return {
        account_names: {},
        sync_warning: 'Credential saved, but some repos could not be synced. Check logs.',
      }
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => screen.getByLabelText(/API key/i))
    fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'test-key' } })
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => {
      const warning = screen.getByRole('alert')
      expect(warning).toHaveTextContent(/some repos could not be synced/i)
    })
  })

  it('shows plain success (no warning) when sync_warning is absent', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return []
      if (cmd === 'get_scheduler_account_names') return {}
      if (cmd === 'save_scheduler_credential') return { account_names: {}, sync_warning: null }
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => screen.getByLabelText(/API key/i))
    fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'test-key' } })
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i })[0])
    await waitFor(() => screen.getByRole('status'))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })
})
