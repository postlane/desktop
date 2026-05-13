// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { invoke } from '../ipc/invoke'
import SchedulerBlock from './SchedulerBlock'

const mockInvoke = vi.mocked(invoke)

function makeProfile(overrides: { provider?: string; connected?: boolean } = {}) {
  return { provider: 'zernio', connected: true, ...overrides }
}

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_scheduler_profiles') return [makeProfile()]
    return null
  })
})

// ── Loading profiles ───────────────────────────────────────────────────────────

describe('SchedulerBlock — list', () => {
  it('calls list_scheduler_profiles on mount', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('list_scheduler_profiles', { projectId: 'proj-1' }))
  })

  it('renders connected scheduler label', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('Zernio')).toBeInTheDocument())
  })

  it('shows empty state when no profiles returned', async () => {
    mockInvoke.mockResolvedValue([])
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText(/No scheduler connected/i)).toBeInTheDocument())
  })

  it('shows Connect button for unconnected provider (not empty state)', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_scheduler_profiles') return [makeProfile({ connected: false })]
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('button', { name: /^Connect$/i })).toBeInTheDocument())
    expect(screen.queryByText(/No scheduler connected/i)).not.toBeInTheDocument()
  })

  it('hides API key form when provider is already connected', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('Zernio')).toBeInTheDocument())
    expect(screen.queryByLabelText(/API key/i)).not.toBeInTheDocument()
  })
})

// ── Remove ─────────────────────────────────────────────────────────────────────

describe('SchedulerBlock — remove', () => {
  it('calls remove_scheduler_credential with correct args', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_scheduler_profiles') return [makeProfile()]
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Disconnect/i }))
    fireEvent.click(screen.getByRole('button', { name: /Disconnect/i }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('remove_scheduler_credential', { provider: 'zernio', projectId: 'proj-1' }))
  })

  it('re-fetches profiles after disconnect', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_scheduler_profiles') return [makeProfile()]
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Disconnect/i }))
    fireEvent.click(screen.getByRole('button', { name: /Disconnect/i }))
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter((c) => c[0] === 'list_scheduler_profiles')
      expect(calls.length).toBeGreaterThanOrEqual(2)
    })
  })

  it('hides Disconnect button for non-owners', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={false} />)
    await waitFor(() => expect(screen.queryByRole('button', { name: /Disconnect/i })).not.toBeInTheDocument())
  })
})

// ── Connect ────────────────────────────────────────────────────────────────────

describe('SchedulerBlock — connect — form', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_scheduler_profiles') return [makeProfile({ connected: false })]
      return null
    })
  })

  it('shows Connect button for unconnected provider', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('button', { name: /^Connect$/i })).toBeInTheDocument())
  })

  it('clicking Connect reveals API key input as type="password"', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Connect$/i }))
    expect(screen.getByLabelText(/API key/i)).toHaveAttribute('type', 'password')
  })

  it('show/hide toggle switches input type', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getByRole('button', { name: /Show/i }))
    expect(screen.getByLabelText(/API key/i)).toHaveAttribute('type', 'text')
    fireEvent.click(screen.getByRole('button', { name: /Hide/i }))
    expect(screen.getByLabelText(/API key/i)).toHaveAttribute('type', 'password')
  })

  it('calls add_scheduler_credential with correct args', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Connect$/i }))
    fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'sk-test-123' } })
    fireEvent.click(screen.getAllByRole('button', { name: /^Connect$/i }).find(b => !b.hasAttribute('disabled')) ?? screen.getByRole('button', { name: /^Connect$/i }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('add_scheduler_credential', {
      provider: 'zernio', apiKey: 'sk-test-123', projectId: 'proj-1',
    }))
  })

  it('Cancel button hides the API key form', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Connect$/i }))
    expect(screen.getByLabelText(/API key/i)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /Cancel/i }))
    expect(screen.queryByLabelText(/API key/i)).not.toBeInTheDocument()
  })
})

describe('SchedulerBlock — connect — access and errors', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_scheduler_profiles') return [makeProfile({ connected: false })]
      return null
    })
  })

  it('hides Connect button for non-owners', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={false} />)
    await waitFor(() => expect(screen.queryByRole('button', { name: /^Connect$/i })).not.toBeInTheDocument())
    expect(screen.queryByLabelText(/API key/i)).not.toBeInTheDocument()
  })

  it('shows provider label using capitalized name for non-zernio providers', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_scheduler_profiles') return [makeProfile({ provider: 'publer', connected: false })]
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText('Publer')).toBeInTheDocument())
  })

  it('shows error alert when add_scheduler_credential fails', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_scheduler_profiles') return [makeProfile({ connected: false })]
      if (cmd === 'add_scheduler_credential') throw new Error('Bad API key')
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /^Connect$/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Connect$/i }))
    const input = await screen.findByLabelText(/API key/i)
    fireEvent.change(input, { target: { value: 'bad-key' } })
    fireEvent.click(screen.getByRole('button', { name: /^Connect$/ }))
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
  })

  it('shows empty profiles when list_scheduler_profiles fails', async () => {
    mockInvoke.mockRejectedValue(new Error('IPC error'))
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText(/No scheduler connected/i)).toBeInTheDocument())
  })
})
