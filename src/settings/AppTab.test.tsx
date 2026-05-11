// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('../TimezoneContext', () => ({
  useTimezone: () => 'UTC',
  getTimezoneOffsetLabel: () => 'UTC+0',
}))
vi.mock('./LicenseSection', () => ({ LicenseSection: () => null }))

import { invoke } from '../ipc/invoke'
import AppTab from './AppTab'

const mockInvoke = vi.mocked(invoke)

function makeAppState(overrides = {}) {
  return {
    version: 1,
    window: { width: 1100, height: 700, x: 0, y: 0 },
    nav: { last_view: 'all_repos', last_repo_id: null, last_section: 'drafts', expanded_repos: [] },
    wizard_completed: true,
    timezone: 'UTC',
    telemetry_consent: false,
    consent_asked: true,
    default_post_time: null,
    ...overrides,
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation((cmd: unknown) => {
    if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
    if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
    if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
    if (cmd === 'get_attribution') return Promise.resolve(true)
    if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
    return Promise.resolve(null)
  })
})

describe('AppTab — default post time — parseInt NaN guard', () => {
  it('does not save NaN when the empty "--" hour option is selected', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({
        default_post_time: { hour: 9, minute: 30, timezone: 'UTC' },
      }))
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      return Promise.resolve(null)
    })

    render(<AppTab />)

    await waitFor(() => {
      const hourSelect = screen.getByRole('combobox', { name: /Default post time hour/i })
      expect(hourSelect).toHaveValue('9')
    })

    const hourSelect = screen.getByRole('combobox', { name: /Default post time hour/i })
    fireEvent.change(hourSelect, { target: { value: '' } })

    await waitFor(() => {
      const saveCalls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'set_default_post_time')
      if (saveCalls.length > 0) {
        const [, arg] = saveCalls[saveCalls.length - 1]
        const typedArg = arg as { dpt: { hour: number } | null }
        expect(typedArg.dpt, 'selecting "--" must save null or a non-NaN hour, not NaN').toSatisfy(
          (dpt: { hour: number } | null) => dpt === null || !isNaN(dpt.hour)
        )
      }
    })
  })

  it('does not save NaN when the empty "--" minute option is selected', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({
        default_post_time: { hour: 9, minute: 30, timezone: 'UTC' },
      }))
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      return Promise.resolve(null)
    })

    render(<AppTab />)

    await waitFor(() => {
      const minuteSelect = screen.getByRole('combobox', { name: /Default post time minute/i })
      expect(minuteSelect).toHaveValue('30')
    })

    const minuteSelect = screen.getByRole('combobox', { name: /Default post time minute/i })
    fireEvent.change(minuteSelect, { target: { value: '' } })

    await waitFor(() => {
      const saveCalls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'set_default_post_time')
      if (saveCalls.length > 0) {
        const [, arg] = saveCalls[saveCalls.length - 1]
        const typedArg = arg as { dpt: { minute: number } | null }
        expect(typedArg.dpt, 'selecting "--" must save null or a non-NaN minute, not NaN').toSatisfy(
          (dpt: { minute: number } | null) => dpt === null || !isNaN(dpt.minute)
        )
      }
    })
  })
})
