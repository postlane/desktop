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
import userEvent from '@testing-library/user-event'

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

describe('AppTab — initial load', () => {
  it('test_displays_version_after_load', async () => {
    render(<AppTab />)
    await waitFor(() => expect(screen.getByText(/postlane 1\.0\.0/i)).toBeInTheDocument())
  })

  it('test_autostart_checkbox_reflects_backend_value', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(true)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('checkbox', { name: /launch at login/i })).toBeChecked())
  })

  it('test_attribution_switch_reflects_backend_false', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(false)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() =>
      expect(screen.getByRole('switch', { name: /post attribution/i })).toHaveAttribute('aria-checked', 'false')
    )
  })

  it('test_telemetry_checkbox_reflects_backend_true', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(true)
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('checkbox', { name: /send anonymous usage data/i })).toBeChecked())
  })

  it('test_default_post_time_loaded_from_app_state', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ default_post_time: { hour: 14, minute: 30, timezone: 'UTC' } }))
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('combobox', { name: /default post time hour/i })).toHaveValue('14'))
    expect(screen.getByRole('combobox', { name: /default post time minute/i })).toHaveValue('30')
  })
})

describe('AppTab — attribution toggle', () => {
  it('test_attribution_toggle_invokes_set_attribution_with_toggled_value', async () => {
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('switch', { name: /post attribution/i })).toBeInTheDocument())
    await userEvent.setup().click(screen.getByRole('switch', { name: /post attribution/i }))
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'set_attribution')
      expect(calls.length).toBe(1)
      expect((calls[0][1] as { enabled: boolean }).enabled).toBe(false)
    })
  })

  it('test_attribution_toggle_ipc_error_shows_alert', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      if (cmd === 'set_attribution') return Promise.reject(new Error('Keyring unavailable'))
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('switch', { name: /post attribution/i })).toBeInTheDocument())
    await userEvent.setup().click(screen.getByRole('switch', { name: /post attribution/i }))
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent(/keyring unavailable/i))
  })
})

describe('AppTab — telemetry toggle', () => {
  it('test_telemetry_toggle_invokes_set_telemetry_consent', async () => {
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('checkbox', { name: /send anonymous usage data/i })).toBeInTheDocument())
    await userEvent.setup().click(screen.getByRole('checkbox', { name: /send anonymous usage data/i }))
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'set_telemetry_consent')
      expect(calls.length).toBe(1)
      expect((calls[0][1] as { consent: boolean }).consent).toBe(true)
    })
  })

  it('test_telemetry_toggle_ipc_error_shows_alert', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      if (cmd === 'set_telemetry_consent') return Promise.reject(new Error('IPC failed'))
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('checkbox', { name: /send anonymous usage data/i })).toBeInTheDocument())
    await userEvent.setup().click(screen.getByRole('checkbox', { name: /send anonymous usage data/i }))
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent(/ipc failed/i))
  })
})

describe('AppTab — autostart toggle', () => {
  it('test_autostart_enable_calls_plugin_enable', async () => {
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('checkbox', { name: /launch at login/i })).not.toBeChecked())
    await userEvent.setup().click(screen.getByRole('checkbox', { name: /launch at login/i }))
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'plugin:autostart|enable')
      expect(calls.length).toBe(1)
    })
  })

  it('test_autostart_disable_calls_plugin_disable', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(true)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('checkbox', { name: /launch at login/i })).toBeChecked())
    await userEvent.setup().click(screen.getByRole('checkbox', { name: /launch at login/i }))
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'plugin:autostart|disable')
      expect(calls.length).toBe(1)
    })
  })
})

describe('AppTab — timezone change', () => {
  it('test_timezone_change_saves_to_app_state_and_calls_callback', async () => {
    const onTimezoneChange = vi.fn()
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      if (cmd === 'save_app_state_command') return Promise.resolve(null)
      return Promise.resolve(null)
    })
    render(<AppTab onTimezoneChange={onTimezoneChange} />)
    await waitFor(() => expect(screen.getByRole('combobox', { name: /display timezone/i })).toBeInTheDocument())
    fireEvent.change(screen.getByRole('combobox', { name: /display timezone/i }), { target: { value: 'America/New_York' } })
    await waitFor(() => {
      const saves = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'save_app_state_command')
      expect(saves.length).toBeGreaterThan(0)
      const [, arg] = saves[saves.length - 1]
      expect((arg as { state: { timezone: string } }).state.timezone).toBe('America/New_York')
    })
    expect(onTimezoneChange).toHaveBeenCalledWith('America/New_York')
  })
})

describe('AppTab — open logs', () => {
  it('test_open_logs_calls_opener_plugin', async () => {
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('button', { name: /open log folder/i })).toBeInTheDocument())
    await userEvent.setup().click(screen.getByRole('button', { name: /open log folder/i }))
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'plugin:opener|open_path')
      expect(calls.length).toBe(1)
    })
  })
})

describe('AppTab — check for updates', () => {
  it('test_check_updates_shows_up_to_date_when_no_update', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      if (cmd === 'plugin:updater|check') return Promise.resolve(null)
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('button', { name: /check for updates/i })).toBeInTheDocument())
    await userEvent.setup().click(screen.getByRole('button', { name: /check for updates/i }))
    await waitFor(() => expect(screen.getByText(/you are up to date/i)).toBeInTheDocument())
  })

  it('test_check_updates_shows_available_version', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      if (cmd === 'plugin:updater|check') return Promise.resolve('1.1.0')
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('button', { name: /check for updates/i })).toBeInTheDocument())
    await userEvent.setup().click(screen.getByRole('button', { name: /check for updates/i }))
    await waitFor(() => expect(screen.getByText(/update available: 1\.1\.0/i)).toBeInTheDocument())
  })

  it('test_check_updates_shows_error_on_failure', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      if (cmd === 'plugin:updater|check') return Promise.reject(new Error('network error'))
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('button', { name: /check for updates/i })).toBeInTheDocument())
    await userEvent.setup().click(screen.getByRole('button', { name: /check for updates/i }))
    await waitFor(() => expect(screen.getByText(/could not check for updates/i)).toBeInTheDocument())
  })
})

describe('AppTab — default post time — hour and minute', () => {
  it('test_hour_change_saves_with_existing_minute', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ default_post_time: { hour: 9, minute: 30, timezone: 'UTC' } }))
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('combobox', { name: /default post time hour/i })).toHaveValue('9'))
    fireEvent.change(screen.getByRole('combobox', { name: /default post time hour/i }), { target: { value: '14' } })
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'set_default_post_time')
      expect(calls.length).toBeGreaterThan(0)
      const [, arg] = calls[calls.length - 1]
      const dpt = (arg as { dpt: { hour: number; minute: number } }).dpt
      expect(dpt.hour).toBe(14)
      expect(dpt.minute).toBe(30)
    })
  })

  it('test_minute_change_saves_with_existing_hour', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ default_post_time: { hour: 9, minute: 30, timezone: 'UTC' } }))
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('combobox', { name: /default post time minute/i })).toHaveValue('30'))
    fireEvent.change(screen.getByRole('combobox', { name: /default post time minute/i }), { target: { value: '45' } })
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'set_default_post_time')
      expect(calls.length).toBeGreaterThan(0)
      const [, arg] = calls[calls.length - 1]
      const dpt = (arg as { dpt: { hour: number; minute: number } }).dpt
      expect(dpt.hour).toBe(9)
      expect(dpt.minute).toBe(45)
    })
  })
})

describe('AppTab — default post time — clear', () => {
  it('test_clear_button_saves_null', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ default_post_time: { hour: 9, minute: 30, timezone: 'UTC' } }))
      if (cmd === 'get_app_version') return Promise.resolve('1.0.0')
      if (cmd === 'get_autostart_enabled') return Promise.resolve(false)
      if (cmd === 'get_attribution') return Promise.resolve(true)
      if (cmd === 'get_telemetry_consent') return Promise.resolve(false)
      return Promise.resolve(null)
    })
    render(<AppTab />)
    await waitFor(() => expect(screen.getByRole('button', { name: /clear default post time/i })).toBeInTheDocument())
    await userEvent.setup().click(screen.getByRole('button', { name: /clear default post time/i }))
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'set_default_post_time')
      expect(calls.length).toBeGreaterThan(0)
      const [, arg] = calls[calls.length - 1]
      expect((arg as { dpt: null }).dpt).toBeNull()
    })
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

describe('AppTab — copy log path (22.9.10c)', () => {
  it('renders Copy log path button; click calls get_log_path then clipboard', async () => {
    const logPath = '/Users/hugo/.postlane/app.log'
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === 'get_log_path' ? Promise.resolve(logPath) : Promise.resolve('1.4.0'))
    render(<AppTab />)
    expect(screen.getByTestId('copy-log-path')).toBeInTheDocument()
    fireEvent.click(screen.getByTestId('copy-log-path'))
    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === 'get_log_path')).toBe(true)
      expect(mockInvoke.mock.calls.some(([cmd, args]) =>
        cmd === 'plugin:clipboard-manager|write_text' && (args as { text: string }).text === logPath
      )).toBe(true)
    })
  })
})
