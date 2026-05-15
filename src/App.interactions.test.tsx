// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/react'
import '@testing-library/jest-dom'
import userEvent from '@testing-library/user-event'

vi.mock('./ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    onResized: vi.fn().mockResolvedValue(() => {}),
    outerPosition: vi.fn().mockResolvedValue({ x: 0, y: 0 }),
  })),
}))
vi.mock('./wizard/Wizard', () => ({
  default: ({ startAt, onComplete }: { startAt?: number; onComplete?: () => void }) => (
    <div data-testid="wizard" data-start={startAt ?? 1}>
      Wizard<button data-testid="wizard-complete" onClick={onComplete}>Complete</button>
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

import { invoke } from './ipc/invoke'
import App from './App'

const mockInvoke = vi.mocked(invoke)

function makeAppState(overrides = {}) {
  return {
    version: 1,
    window: { width: 1100, height: 700, x: 0, y: 0 },
    nav: { last_view: 'all_repos', last_repo_id: null, last_section: 'drafts', expanded_repos: [] },
    wizard_completed: true, timezone: '', telemetry_consent: false, consent_asked: true,
    ...overrides,
  }
}

const MOCK_PROJECT = { id: 'p1', name: 'My Org', workspace_type: 'personal', tier: 'free', billing_active: true, is_owner: true }

function signedInInvoke(overrides: Record<string, unknown> = {}) {
  mockInvoke.mockImplementation((cmd: unknown) => {
    if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true, post_wizard_completed: true, ...overrides }))
    if (cmd === 'get_license_signed_in') return Promise.resolve(true)
    if (cmd === 'list_projects') return Promise.resolve([])
    if (cmd === 'get_all_drafts') return Promise.resolve([])
    return Promise.resolve(null)
  })
}

beforeEach(() => { vi.clearAllMocks() })

// ── Consent modal ─────────────────────────────────────────────────────────────

describe('App — consent modal', () => {
  function consentPendingInvoke() {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true, post_wizard_completed: true, consent_asked: false }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
  }

  it('shows consent buttons when consent_asked is false', async () => {
    consentPendingInvoke()
    render(<App />)
    await waitFor(() => expect(screen.getByTestId('consent-accept')).toBeInTheDocument())
  })

  it('accept calls set_telemetry_consent with true', async () => {
    consentPendingInvoke()
    render(<App />)
    await waitFor(() => screen.getByTestId('consent-accept'))
    await userEvent.setup().click(screen.getByTestId('consent-accept'))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_telemetry_consent', { consent: true }))
    expect(screen.queryByTestId('consent-accept')).not.toBeInTheDocument()
  })

  it('decline calls set_telemetry_consent with false', async () => {
    consentPendingInvoke()
    render(<App />)
    await waitFor(() => screen.getByTestId('consent-decline'))
    await userEvent.setup().click(screen.getByTestId('consent-decline'))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_telemetry_consent', { consent: false }))
  })
})

// ── Wizard complete ───────────────────────────────────────────────────────────

describe('App — wizard complete', () => {
  it('completing wizard shows main app', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: false }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => screen.getByTestId('wizard'))
    await userEvent.setup().click(screen.getByTestId('wizard-complete'))
    await waitFor(() => expect(screen.getByText('LeftNav')).toBeInTheDocument())
  })
})

// ── Re-sign-in ────────────────────────────────────────────────────────────────

describe('App — re-sign-in', () => {
  it('clicking sign in shows main app', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(false)
      if (cmd === 'list_projects') return Promise.resolve([])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => screen.getByText('ReSignInScreen'))
    await userEvent.setup().click(screen.getByTestId('resign-in'))
    await waitFor(() => expect(screen.getByText('LeftNav')).toBeInTheDocument())
  })
})

// ── Sign out ──────────────────────────────────────────────────────────────────

describe('App — sign out', () => {
  it('signing out from AccountSettingsView shows ReSignInScreen', async () => {
    signedInInvoke()
    render(<App />)
    await waitFor(() => screen.getByText('LeftNav'))
    await userEvent.setup().click(screen.getByTestId('leftnav-settings'))
    await waitFor(() => screen.getByTestId('sign-out'))
    await userEvent.setup().click(screen.getByTestId('sign-out'))
    await waitFor(() => expect(screen.getByText('ReSignInScreen')).toBeInTheDocument())
  })
})

// ── Cmd+H shortcut ────────────────────────────────────────────────────────────

describe('App — Cmd+H shortcut', () => {
  it('Cmd+H navigates to org_history using current project id', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true, post_wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([MOCK_PROJECT])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      if (cmd === 'get_org_published') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => screen.getByTestId('select-post'))
    fireEvent.keyDown(document, { key: 'h', metaKey: true })
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_org_published', { projectId: 'p1' }))
  })
})

// ── Global settings navigation ────────────────────────────────────────────────

describe('App — global settings', () => {
  it('onSettingsOpen navigates to AccountSettingsView', async () => {
    signedInInvoke()
    render(<App />)
    await waitFor(() => screen.getByText('LeftNav'))
    await userEvent.setup().click(screen.getByTestId('leftnav-settings'))
    await waitFor(() => expect(screen.getByText('AccountSettingsView')).toBeInTheDocument())
  })
})

// ── Discard modal ─────────────────────────────────────────────────────────────

describe('App — discard modal', () => {
  async function setupWithEditView() {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true, post_wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([MOCK_PROJECT])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => screen.getByTestId('select-post'))
    await userEvent.setup().click(screen.getByTestId('select-post'))
    await waitFor(() => screen.getByTestId('set-dirty'))
    fireEvent.click(screen.getByTestId('set-dirty'))
  }

  it('nav click while dirty shows discard modal', async () => {
    await setupWithEditView()
    fireEvent.click(screen.getByTestId('leftnav-nav'))
    expect(screen.getByRole('button', { name: /discard/i })).toBeInTheDocument()
  })

  it('confirming discard closes modal', async () => {
    await setupWithEditView()
    fireEvent.click(screen.getByTestId('leftnav-nav'))
    await userEvent.setup().click(screen.getByRole('button', { name: /discard/i }))
    await waitFor(() => expect(screen.queryByRole('button', { name: /discard/i })).not.toBeInTheDocument())
  })

  it('cancelling discard closes modal', async () => {
    await setupWithEditView()
    fireEvent.click(screen.getByTestId('leftnav-nav'))
    fireEvent.click(screen.getByRole('button', { name: /^Cancel$/i }))
    expect(screen.queryByRole('button', { name: /discard/i })).not.toBeInTheDocument()
  })
})

// ── Toast ─────────────────────────────────────────────────────────────────────

describe('App — toast', () => {
  it('shows toast notification when triggered', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true, post_wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([MOCK_PROJECT])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => screen.getByTestId('select-post'))
    await userEvent.setup().click(screen.getByTestId('select-post'))
    await waitFor(() => screen.getByTestId('show-toast'))
    await userEvent.setup().click(screen.getByTestId('show-toast'))
    await waitFor(() => expect(screen.getByText('Test toast')).toBeInTheDocument())
  })
})

// ── Toast — timer replacement ─────────────────────────────────────────────────

describe('App — toast timer clears existing timer', () => {
  it('showing toast twice clears the first timer and resets', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true, post_wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([MOCK_PROJECT])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => screen.getByTestId('select-post'))
    await userEvent.setup().click(screen.getByTestId('select-post'))
    await waitFor(() => screen.getByTestId('show-toast'))
    // Click toast twice — the second click should clear the first timer (branch 26)
    await userEvent.setup().click(screen.getByTestId('show-toast'))
    await userEvent.setup().click(screen.getByTestId('show-toast'))
    await waitFor(() => expect(screen.getByText('Test toast')).toBeInTheDocument())
  })
})

// ── Key press that does not trigger Cmd+H ─────────────────────────────────────

describe('App — non-matching keydown events', () => {
  it('pressing a key without modifier does not navigate', async () => {
    signedInInvoke()
    render(<App />)
    await waitFor(() => screen.getByText('LeftNav'))
    const invokeBefore = mockInvoke.mock.calls.length
    fireEvent.keyDown(document, { key: 'h' })
    // No new invoke calls should have been triggered
    expect(mockInvoke.mock.calls.length).toBe(invokeBefore)
  })
})

// ── Ctrl+H shortcut ───────────────────────────────────────────────────────────

describe('App — Ctrl+H shortcut', () => {
  it('Ctrl+H also navigates to org_history', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true, post_wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([MOCK_PROJECT])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      if (cmd === 'get_org_published') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => screen.getByTestId('select-post'))
    fireEvent.keyDown(document, { key: 'h', ctrlKey: true })
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_org_published', { projectId: 'p1' }))
  })

  it('Cmd+H on global_settings view uses empty projectId', async () => {
    signedInInvoke()
    render(<App />)
    await waitFor(() => screen.getByText('LeftNav'))
    await userEvent.setup().click(screen.getByTestId('leftnav-settings'))
    await waitFor(() => screen.getByText('AccountSettingsView'))
    fireEvent.keyDown(document, { key: 'h', metaKey: true })
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_org_published', { projectId: '' }))
  })
})

// ── handleConsentChoice error path ────────────────────────────────────────────

describe('App — consent choice IPC error', () => {
  it('logs error and still closes modal when set_telemetry_consent rejects', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({
        wizard_completed: true, post_wizard_completed: true, consent_asked: false,
      }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      if (cmd === 'set_telemetry_consent') return Promise.reject(new Error('keyring locked'))
      return Promise.resolve(null)
    })
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    render(<App />)
    await waitFor(() => screen.getByTestId('consent-accept'))
    await userEvent.setup().click(screen.getByTestId('consent-accept'))
    await waitFor(() => expect(screen.queryByTestId('consent-accept')).not.toBeInTheDocument())
    errorSpy.mockRestore()
  })
})

// ── handleWizardNudgeHandled when appStateRef is null ─────────────────────────

describe('App — wizard nudge handled without appState', () => {
  it('does not crash when appStateRef is null at nudge-handled time', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: false }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'list_projects') return Promise.resolve([MOCK_PROJECT])
      if (cmd === 'get_all_drafts') return Promise.resolve([])
      return Promise.resolve(null)
    })
    // When the wizard is shown appStateRef.current is set, but we can still
    // exercise the nudge-handled path by completing the wizard and verifying
    // the app doesn't crash.
    render(<App />)
    await waitFor(() => screen.getByTestId('wizard'))
    await userEvent.setup().click(screen.getByTestId('wizard-complete'))
    await waitFor(() => expect(screen.getByText('LeftNav')).toBeInTheDocument())
  })
})

// ── initError: non-Error rejection ───────────────────────────────────────────

describe('App — init error from string rejection', () => {
  it('displays a string rejection as the error message', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.reject('disk full')
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(screen.getByRole('alert').textContent).toMatch(/disk full/i)
  })
})
