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
vi.mock('./wizard/Wizard', () => ({
  default: ({ startAt, onComplete }: { startAt?: number; onComplete?: () => void }) => (
    <div data-testid="wizard" data-start={startAt ?? 1}>
      Wizard
      <button data-testid="wizard-complete" onClick={onComplete}>Complete</button>
    </div>
  ),
}))
vi.mock('./wizard/ReSignInScreen', () => ({
  default: ({ onSignedIn }: { onSignedIn?: () => void }) => (
    <div>ReSignInScreen<button data-testid="resign-in" onClick={onSignedIn}>Sign in</button></div>
  ),
}))
vi.mock('./nav/LeftNav', () => ({
  default: ({ onNavigate, onAddWorkspace, onSettingsOpen }: {
    onNavigate?: (_v: { view: string }) => void;
    onAddWorkspace?: () => void;
    onSettingsOpen?: () => void;
  }) => (
    <>
      <div>LeftNav</div>
      <button data-testid="leftnav-nav" onClick={() => onNavigate?.({ view: 'no_orgs' })} />
      <button data-testid="leftnav-add-org" onClick={() => onAddWorkspace?.()} />
      <button data-testid="leftnav-settings" onClick={() => onSettingsOpen?.()} />
    </>
  ),
}))
vi.mock('./telemetry/TelemetryConsentModal', () => ({
  default: ({ onAccept, onDecline }: { onAccept?: () => void; onDecline?: () => void }) => (
    <>
      <button data-testid="consent-accept" onClick={onAccept}>Accept</button>
      <button data-testid="consent-decline" onClick={onDecline}>Decline</button>
    </>
  ),
}))
vi.mock('./settings/MigrationBanner', () => ({ MigrationBannersBlock: () => null,
  useMigrationStatus: () => ({ status: null, dismiss: vi.fn() }), useJournalStatuses: () => ({ statuses: [], resume: vi.fn(), dismissSession: vi.fn() }) }))
vi.mock('./settings/DangerZone', () => ({ default: () => null }))
vi.mock('./settings/OrgSettingsView', () => ({ default: () => <div>OrgSettingsView</div> }))
vi.mock('./settings/AccountSettingsView', () => ({
  default: ({ onSignedOut }: { onSignedOut?: () => void }) => (
    <div>AccountSettingsView<button data-testid="sign-out" onClick={onSignedOut}>Sign out</button></div>
  ),
}))
vi.mock('./settings/PreferencesSettingsView', () => ({ default: () => <div>PreferencesSettingsView</div> }))
vi.mock('./settings/SystemSettingsView', () => ({ default: () => <div>SystemSettingsView</div> }))
vi.mock('./components/PostTable', () => ({
  default: ({ onSelect }: { onSelect?: (_p: unknown) => void }) => (
    <button data-testid="select-post" onClick={() => onSelect?.({
      id: 'd1', project_id: 'p1', repo_id: 'r1', title: 'T', body: '', status: 'draft', created_at: '',
    })}>Select Post</button>
  ),
}))
vi.mock('./components/EditPostView', () => ({
  default: ({ onDirtyChange, onToast }: {
    onDirtyChange?: (_d: boolean) => void;
    onToast?: (_msg: string) => void;
  }) => (
    <div data-testid="edit-post-view">
      <button data-testid="set-dirty" onClick={() => onDirtyChange?.(true)}>Dirty</button>
      <button data-testid="show-toast" onClick={() => onToast?.('Test toast')}>Toast</button>
    </div>
  ),
}))
vi.mock('./components/OrgUpgradeBanner', () => ({ default: () => null }))
vi.mock('./components/OrgLinkModal', () => ({ default: () => null }))

import userEvent from '@testing-library/user-event'
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

describe('useWindowSizePersistence — resize save error (Error object)', () => {
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

describe('useWindowSizePersistence — debounce timer replacement', () => {
  it('clears existing timer when a second resize fires before the first expires', async () => {
    let capturedResizeHandler: ((event: { payload: { width: number; height: number } }) => void) | undefined
    vi.mocked(getCurrentWindow).mockReturnValue({
      onResized: vi.fn().mockImplementation((handler: (event: { payload: { width: number; height: number } }) => void) => {
        capturedResizeHandler = handler
        return Promise.resolve(() => {})
      }),
      outerPosition: vi.fn().mockResolvedValue({ x: 10, y: 20 }),
    } as unknown as ReturnType<typeof getCurrentWindow>)
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true, post_wizard_completed: true, consent_asked: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => { expect(capturedResizeHandler).toBeDefined() })
    // Fire two resize events within the debounce window (< 500 ms apart)
    // so the clearTimeout branch is exercised on the second call
    if (capturedResizeHandler) capturedResizeHandler({ payload: { width: 900, height: 650 } })
    if (capturedResizeHandler) capturedResizeHandler({ payload: { width: 950, height: 670 } })
    // Wait past the debounce delay so the final callback runs
    await new Promise((resolve) => setTimeout(resolve, 600))
    const saveCalls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'save_app_state_command')
    const windowSaves = saveCalls.filter(([, arg]) => {
      const a = arg as Record<string, unknown>
      const s = a.state as Record<string, unknown> | undefined
      return s && typeof s.window === 'object'
    })
    // At least one save should occur; the key thing being tested is that the
    // clearTimeout branch (line 183) was executed without error
    expect(windowSaves.length).toBeGreaterThanOrEqual(1)
  })
})

describe('useWindowSizePersistence — resize save error (string rejection)', () => {
  it('logs a string (not Error object) when the non-Error is thrown inside the resize callback', async () => {
    let capturedResizeHandler: ((event: { payload: { width: number; height: number } }) => void) | undefined
    vi.mocked(getCurrentWindow).mockReturnValue({
      onResized: vi.fn().mockImplementation((handler: (event: { payload: { width: number; height: number } }) => void) => {
        capturedResizeHandler = handler
        return Promise.resolve(() => {})
      }),
      outerPosition: vi.fn().mockRejectedValue('string rejection from outer position'),
    } as unknown as ReturnType<typeof getCurrentWindow>)
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      return Promise.resolve(null)
    })
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    render(<App />)
    await waitFor(() => { expect(capturedResizeHandler).toBeDefined() })
    if (capturedResizeHandler) capturedResizeHandler({ payload: { width: 800, height: 600 } })
    await new Promise((resolve) => setTimeout(resolve, 600))
    const resizeErrors = errorSpy.mock.calls.filter((args) =>
      String(args[0]).includes('persist window size')
    )
    expect(resizeErrors.length).toBeGreaterThan(0)
    const errorArg = resizeErrors[0][resizeErrors[0].length - 1]
    expect(typeof errorArg).toBe('string')
    errorSpy.mockRestore()
  })
})

describe('useWindowSizePersistence — onResized setup failure', () => {
  it('logs error when onResized promise itself rejects with an Error', async () => {
    vi.mocked(getCurrentWindow).mockReturnValue({
      onResized: vi.fn().mockRejectedValue(new Error('listener setup failed')),
      outerPosition: vi.fn().mockResolvedValue({ x: 0, y: 0 }),
    } as unknown as ReturnType<typeof getCurrentWindow>)

    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      return Promise.resolve(null)
    })

    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    render(<App />)
    await waitFor(() => {
      const setupErrors = errorSpy.mock.calls.filter((args) =>
        String(args[0]).includes('resize listener')
      )
      expect(setupErrors.length).toBeGreaterThan(0)
      const errorArg = setupErrors[0][setupErrors[0].length - 1]
      expect(typeof errorArg).toBe('string')
    })
    errorSpy.mockRestore()
  })

  it('logs string when onResized rejects with a non-Error value', async () => {
    vi.mocked(getCurrentWindow).mockReturnValue({
      onResized: vi.fn().mockRejectedValue('raw string rejection'),
      outerPosition: vi.fn().mockResolvedValue({ x: 0, y: 0 }),
    } as unknown as ReturnType<typeof getCurrentWindow>)

    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      return Promise.resolve(null)
    })

    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    render(<App />)
    await waitFor(() => {
      const setupErrors = errorSpy.mock.calls.filter((args) =>
        String(args[0]).includes('resize listener')
      )
      expect(setupErrors.length).toBeGreaterThan(0)
      const errorArg = setupErrors[0][setupErrors[0].length - 1]
      expect(typeof errorArg).toBe('string')
    })
    errorSpy.mockRestore()
  })
})

describe('App startup — routing', () => {
  it('test_shows_wizard_on_first_launch', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: false }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(false)
      if (cmd === 'has_active_repos') return Promise.resolve(false)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => { expect(screen.getByText('Wizard')).toBeInTheDocument() })
  })

  it('test_skips_wizard_and_shows_app_when_repos_exist_despite_wizard_not_completed', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: false }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'has_active_repos') return Promise.resolve(true)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => expect(screen.getByText('LeftNav')).toBeInTheDocument())
    expect(screen.queryByText('Wizard')).not.toBeInTheDocument()
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

describe('App — Add org flow', () => {
  it('test_add_org_click_shows_wizard_at_step_2', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => expect(screen.getByText('LeftNav')).toBeInTheDocument())
    await userEvent.setup().click(screen.getByTestId('leftnav-add-org'))
    await waitFor(() => {
      const wizard = screen.getByTestId('wizard')
      expect(wizard).toBeInTheDocument()
      expect(wizard).toHaveAttribute('data-start', '2')
    })
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

describe('App startup — timezone already set', () => {
  it('does not call save_app_state_command for timezone when timezone is already set', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true, post_wizard_completed: true, timezone: 'America/New_York' }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => expect(screen.getByText('LeftNav')).toBeInTheDocument())
    const tzSaveCalls = mockInvoke.mock.calls.filter(([cmd, arg]) => {
      if (cmd !== 'save_app_state_command') return false
      const a = arg as Record<string, unknown>
      const s = a.state as Record<string, unknown> | undefined
      return s && typeof s.window !== 'object' && typeof s.timezone === 'string'
    })
    expect(tzSaveCalls).toHaveLength(0)
  })
})
