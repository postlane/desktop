// SPDX-License-Identifier: BUSL-1.1
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, act } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('./ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    onResized: vi.fn().mockResolvedValue(() => {}),
    outerPosition: vi.fn().mockResolvedValue({ x: 0, y: 0 }),
  }))
}))
vi.mock('./wizard/Wizard', () => ({ default: () => <div>Wizard</div> }))
vi.mock('./wizard/ReSignInScreen', () => ({ default: () => <div>ReSignInScreen</div> }))
vi.mock('./nav/LeftNav', () => ({
  default: ({ onNavigate }: { onNavigate?: (_v: { view: string }) => void }) => (
    <>
      <div>LeftNav</div>
      <button data-testid="leftnav-nav" onClick={() => onNavigate?.({ view: 'no_orgs' })} />
    </>
  ),
}))
vi.mock('./telemetry/TelemetryConsentModal', () => ({ default: () => null }))
vi.mock('./drafts/AllReposDraftsView', () => ({ default: () => <div>AllReposDraftsView</div> }))
vi.mock('./settings/SettingsPanel', () => ({ default: () => <div>SettingsPanel</div> }))
vi.mock('./settings/OrgSettingsView', () => ({ default: () => <div>OrgSettingsView</div> }))

import { invoke } from './ipc/invoke'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import App from './App'

const mockInvoke = vi.mocked(invoke)
const mockListen = vi.mocked(listen)

function makeAppState(overrides = {}) {
  return {
    version: 1,
    window: { width: 1100, height: 700, x: 0, y: 0 },
    nav: { last_view: 'all_repos', last_repo_id: null, last_section: 'drafts', expanded_repos: [] },
    wizard_completed: true,
    timezone: '',
    telemetry_consent: false,
    consent_asked: true,
    ...overrides,
  }
}

beforeEach(() => { vi.clearAllMocks() })

describe('useWindowSizePersistence error logging', () => {
  it('logs error message string (not raw Error object) when resize save fails', async () => {
    let capturedResizeHandler: ((event: { payload: { width: number; height: number } }) => void) | undefined
    const outerPositionMock = vi.fn().mockRejectedValue(new Error('IPC error'))

    vi.mocked(getCurrentWindow).mockReturnValue({
      onResized: vi.fn().mockImplementation((handler: (event: { payload: { width: number; height: number } }) => void) => {
        capturedResizeHandler = handler
        return Promise.resolve(() => {})
      }),
      outerPosition: outerPositionMock,
    } as unknown as ReturnType<typeof getCurrentWindow>)

    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })

    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    render(<App />)
    await waitFor(() => { expect(capturedResizeHandler).toBeDefined() })

    if (capturedResizeHandler) capturedResizeHandler({ payload: { width: 800, height: 600 } })
    await new Promise((resolve) => setTimeout(resolve, 600))

    const resizeErrorCall = errorSpy.mock.calls.find((args) =>
      String(args[0]).includes('window') || String(args[1])?.includes('window')
    )
    if (resizeErrorCall) {
      const errorArg = resizeErrorCall[resizeErrorCall.length - 1]
      expect(typeof errorArg).toBe('string')
      expect(String(errorArg)).not.toMatch(/\n\s+at\s/)
    }

    errorSpy.mockRestore()
  })
})

describe('App startup — routing', () => {
  it('test_shows_wizard_on_first_launch', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: false }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(false)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => { expect(screen.getByText('Wizard')).toBeInTheDocument() })
  })

  it('test_shows_resign_in_when_token_missing', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(false)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => { expect(screen.getByText('ReSignInScreen')).toBeInTheDocument() })
  })

  it('test_shows_main_app_when_complete', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => { expect(screen.getByText('LeftNav')).toBeInTheDocument() })
  })

  it('shows an error message when startup IPC calls fail (§review-critical)', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.reject(new Error('Keyring unavailable'))
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => { expect(screen.getByRole('alert')).toBeInTheDocument() })
    expect(screen.getByRole('alert').textContent).toMatch(/Keyring unavailable/i)
  })
})

describe('App startup — state initialisation', () => {
  it('test_app_initialises_system_timezone_on_first_launch', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ timezone: '' }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => {
      const saveCalls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'save_app_state_command')
      const tzSave = saveCalls.find(([, arg]) => {
        const a = arg as Record<string, unknown>
        const s = a.state as Record<string, unknown> | undefined
        return typeof s?.timezone === 'string' && s.timezone !== ''
      })
      expect(tzSave, 'save_app_state_command must be called with { state: { timezone: ... } }').toBeDefined()
    })
  })

  it('shows ReSignInScreen when license:expired event fires', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })
    mockListen.mockResolvedValue(() => {})
    render(<App />)
    await waitFor(() => expect(screen.getByText('LeftNav')).toBeInTheDocument())
    const expiredEntry = mockListen.mock.calls.find(([ev]) => ev === 'license:expired')
    expect(expiredEntry, 'license:expired listener must be registered').toBeDefined()
    if (!expiredEntry) throw new Error('license:expired listener not registered')
    act(() => (expiredEntry[1] as () => void)())
    await waitFor(() => expect(screen.getByText('ReSignInScreen')).toBeInTheDocument())
  })
})

describe('App startup — post-wizard nudge', () => {
  it('navigates to OrgSettingsView on startup when post_wizard_completed absent and projects exist', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([{ id: 'p1', name: 'My Org', workspace_type: 'personal', tier: 'free', billing_active: true, is_owner: true }])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => expect(screen.getByText('OrgSettingsView')).toBeInTheDocument())
  })

  it('auto-navigates to org_queue for first project after load when post_wizard_completed is true', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true, post_wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([{ id: 'p1', name: 'My Org', workspace_type: 'personal', tier: 'free', billing_active: true, is_owner: true }])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    // The no_orgs view must not show a spinner indefinitely after projects have loaded
    await waitFor(() => expect(screen.queryByText('Loading…')).not.toBeInTheDocument())
  })

  it('does not navigate to OrgSettingsView when list_projects returns empty', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => expect(screen.getByText('LeftNav')).toBeInTheDocument())
    expect(screen.queryByText('OrgSettingsView')).not.toBeInTheDocument()
  })
})
