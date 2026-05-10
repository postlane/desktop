// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { invoke } from '../ipc/invoke'
import PreferencesSettingsView from './PreferencesSettingsView'
import type { AppStateFile } from '../types'

const mockInvoke = vi.mocked(invoke)

function makeAppState(overrides: Partial<AppStateFile> = {}): AppStateFile {
  return {
    version: 1,
    window: { width: 1024, height: 768, x: 0, y: 0 },
    nav: { last_view: '', last_repo_id: null, last_section: '', expanded_repos: [] },
    wizard_completed: true,
    timezone: 'UTC',
    telemetry_consent: true,
    consent_asked: true,
    default_post_time: null,
    notifications_enabled: true,
    ...overrides,
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'get_app_state') return makeAppState()
    return null
  })
})

// ── Load ───────────────────────────────────────────────────────────────────────

describe('PreferencesSettingsView — load', () => {
  it('calls get_app_state on mount', async () => {
    render(<PreferencesSettingsView onTimezoneChange={vi.fn()} />)
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_app_state'))
  })

  it('shows current timezone in the selector', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_app_state') return makeAppState({ timezone: 'America/New_York' })
      return null
    })
    render(<PreferencesSettingsView onTimezoneChange={vi.fn()} />)
    await waitFor(() => {
      const select = screen.getByLabelText(/timezone/i)
      expect(select).toHaveValue('America/New_York')
    })
  })
})

// ── Timezone selector ──────────────────────────────────────────────────────────

describe('PreferencesSettingsView — timezone', () => {
  it('renders timezone as a <select> element', async () => {
    render(<PreferencesSettingsView onTimezoneChange={vi.fn()} />)
    await waitFor(() => expect(screen.getByLabelText(/timezone/i).tagName).toBe('SELECT'))
  })

  it('timezone select contains valid IANA options', async () => {
    render(<PreferencesSettingsView onTimezoneChange={vi.fn()} />)
    await waitFor(() => {
      const select = screen.getByLabelText(/timezone/i) as HTMLSelectElement
      const options = Array.from(select.options).map((o) => o.value)
      expect(options).toContain('Europe/London')
      expect(options).toContain('America/New_York')
    })
  })

  it('saves timezone via save_app_state_command with { state: ... } wrapper', async () => {
    render(<PreferencesSettingsView onTimezoneChange={vi.fn()} />)
    await waitFor(() => screen.getByLabelText(/timezone/i))
    fireEvent.change(screen.getByLabelText(/timezone/i), { target: { value: 'Europe/London' } })
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('save_app_state_command',
      { state: expect.objectContaining({ timezone: 'Europe/London' }) }))
  })

  it('calls onTimezoneChange after saving', async () => {
    const onTzChange = vi.fn()
    render(<PreferencesSettingsView onTimezoneChange={onTzChange} />)
    await waitFor(() => screen.getByLabelText(/timezone/i))
    fireEvent.change(screen.getByLabelText(/timezone/i), { target: { value: 'Asia/Tokyo' } })
    await waitFor(() => expect(onTzChange).toHaveBeenCalledWith('Asia/Tokyo'))
  })
})

// ── Notifications toggle ───────────────────────────────────────────────────────

describe('PreferencesSettingsView — notifications', () => {
  it('renders a notifications toggle', async () => {
    render(<PreferencesSettingsView onTimezoneChange={vi.fn()} />)
    await waitFor(() => expect(screen.getByLabelText(/notifications/i)).toBeInTheDocument())
  })

  it('toggling off saves notifications_enabled: false with { state: ... } wrapper', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_app_state') return makeAppState({ notifications_enabled: true })
      return null
    })
    render(<PreferencesSettingsView onTimezoneChange={vi.fn()} />)
    await waitFor(() => screen.getByLabelText(/notifications/i))
    fireEvent.click(screen.getByLabelText(/notifications/i))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('save_app_state_command',
      { state: expect.objectContaining({ notifications_enabled: false }) }))
  })
})

// ── Theme selector ─────────────────────────────────────────────────────────────

describe('PreferencesSettingsView — theme', () => {
  it('shows a theme selector with Coming soon tooltip', async () => {
    render(<PreferencesSettingsView onTimezoneChange={vi.fn()} />)
    await waitFor(() => {
      const themeEl = screen.getByTitle(/Coming soon/i)
      expect(themeEl).toBeInTheDocument()
    })
  })
})

// ── Autostart toggle ───────────────────────────────────────────────────────────

describe('PreferencesSettingsView — autostart (macOS only)', () => {
  it('hides autostart toggle on non-macOS', async () => {
    render(<PreferencesSettingsView onTimezoneChange={vi.fn()} />)
    await waitFor(() => {})
    expect(screen.queryByLabelText(/Launch at login/i)).not.toBeInTheDocument()
  })
})
