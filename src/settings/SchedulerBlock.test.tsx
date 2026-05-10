// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { invoke } from '../ipc/invoke'
import SchedulerBlock from './SchedulerBlock'

const mockInvoke = vi.mocked(invoke)

function makeProfile(overrides = {}) {
  return { provider: 'zernio', label: 'Zernio', ...overrides }
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

  it('shows empty state when no profiles', async () => {
    mockInvoke.mockResolvedValue([])
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText(/No scheduler connected/i)).toBeInTheDocument())
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

describe('SchedulerBlock — connect', () => {
  it('renders API key input as type="password"', () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    const input = screen.getByLabelText(/API key/i)
    expect(input).toHaveAttribute('type', 'password')
  })

  it('show/hide toggle switches input type', () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    const toggle = screen.getByRole('button', { name: /Show/i })
    fireEvent.click(toggle)
    expect(screen.getByLabelText(/API key/i)).toHaveAttribute('type', 'text')
    fireEvent.click(screen.getByRole('button', { name: /Hide/i }))
    expect(screen.getByLabelText(/API key/i)).toHaveAttribute('type', 'password')
  })

  it('calls add_scheduler_credential with api key', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_scheduler_profiles') return []
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    fireEvent.change(screen.getByLabelText(/API key/i), { target: { value: 'sk-test-123' } })
    fireEvent.click(screen.getByRole('button', { name: /^Connect$/i }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('add_scheduler_credential', {
      provider: 'zernio', apiKey: 'sk-test-123', projectId: 'proj-1',
    }))
  })

  it('hides Connect form for non-owners', () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={false} />)
    expect(screen.queryByLabelText(/API key/i)).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^Connect$/i })).not.toBeInTheDocument()
  })
})
