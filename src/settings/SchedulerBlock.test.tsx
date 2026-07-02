// SPDX-License-Identifier: BUSL-1.1
// Buffer removed as a selectable provider 2026-07-01: Buffer's old REST API
// (api.bufferapp.com, which the shipped BufferProvider targets) is closed to
// new developer app registrations, and Buffer's new GraphQL API doesn't
// support third-party OAuth yet. No new user can obtain the credentials this
// provider needs, so it must not be offered as a connectable option — same
// treatment as Medium in the v2.5 brief. Existing connections (if any) are
// unaffected: this only changes the "add new provider" list, not the
// already-connected list, which is driven by a separate `connected` state.

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))

import { invoke } from '../ipc/invoke'
import SchedulerBlock from './SchedulerBlock'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_connected_providers') return []
    if (cmd === 'get_scheduler_account_names') return {}
    return null
  })
})

describe('SchedulerBlock — Buffer removed from available providers', () => {
  it('does not offer Buffer as a connectable provider', async () => {
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getAllByRole('button', { name: /^Connect$/i }))
    expect(screen.queryByText(/^Buffer$/)).not.toBeInTheDocument()
  })

  it('still shows an already-connected Buffer provider (existing connections unaffected)', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_connected_providers') return ['buffer']
      if (cmd === 'get_scheduler_account_names') return {}
      return null
    })
    render(<SchedulerBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText(/^Buffer$/)).toBeInTheDocument())
  })
})
