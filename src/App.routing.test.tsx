// SPDX-License-Identifier: BUSL-1.1
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
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
  default: ({ onComplete }: { onComplete?: () => void }) => (
    <div data-testid="wizard">Wizard<button data-testid="wizard-complete" onClick={onComplete}>Complete</button></div>
  ),
}))
vi.mock('./wizard/ReSignInScreen', () => ({
  default: () => <div>ReSignInScreen</div>,
}))
vi.mock('./nav/LeftNav', async () => {
  const { useProjectsContext } = await import('./context/ProjectsProvider')
  return {
    default: function MockLeftNav({ onNavigate, onAddWorkspace, onSettingsOpen }: {
      onNavigate?: (_v: { view: string; projectId?: string; section?: string }) => void;
      onAddWorkspace?: () => void;
      onSettingsOpen?: () => void;
    }) {
      const { projects, loading, error, refresh } = useProjectsContext()
      return (
        <>
          <div>LeftNav</div>
          {!loading && !error && projects.length === 0 && (
            <button onClick={onAddWorkspace}>Create your first workspace</button>
          )}
          {!loading && error && <button onClick={refresh}>Retry</button>}
          <button data-testid="leftnav-add-org" onClick={() => onAddWorkspace?.()} />
          <button data-testid="leftnav-settings" onClick={() => onSettingsOpen?.()} />
          <button data-testid="nav-org-history" onClick={() => onNavigate?.({ view: 'org_history', projectId: 'p1' })} />
          <button data-testid="nav-org-settings" onClick={() => onNavigate?.({ view: 'org_settings', projectId: 'p1', section: 'settings' })} />
        </>
      )
    },
  }
})
vi.mock('./telemetry/TelemetryConsentModal', () => ({ default: () => null }))
vi.mock('./settings/MigrationBanner', () => ({
  MigrationBannersBlock: () => null,
  useMigrationStatus: () => ({ status: null, dismiss: vi.fn() }),
  useJournalStatuses: () => ({ statuses: [], resume: vi.fn(), dismissSession: vi.fn() }),
}))
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
vi.mock('./components/EditPostView', async () => {
  const { useEditGuard } = await import('./context/EditGuardContext')
  return {
    default: function EditPostViewMock({ onBack }: { onBack?: () => void }) {
      const { setDirty } = useEditGuard()
      return (
        <div data-testid="edit-post-view">
          <button data-testid="back-from-edit" onClick={() => onBack?.()}>Back</button>
          <button data-testid="dirty-btn" onClick={() => setDirty(true)}>Make dirty</button>
        </div>
      )
    },
  }
})
vi.mock('./components/OrgUpgradeBanner', () => ({ default: () => null }))
vi.mock('./components/OrgLinkModal', () => ({ default: () => null }))

import userEvent from '@testing-library/user-event'
import { invoke } from './ipc/invoke'
import App from './App'

const mockInvoke = vi.mocked(invoke)

function makeAppState(overrides = {}) {
  return {
    version: 1,
    window: { width: 1100, height: 700, x: 0, y: 0 },
    nav: { last_view: 'all_repos', last_repo_id: null, last_section: 'drafts', expanded_repos: [] },
    wizard_completed: true,
    timezone: 'America/New_York',
    telemetry_consent: true,
    consent_asked: true,
    ...overrides,
  }
}

function makeProject(id = 'p1') {
  return { id, name: 'My Org', workspace_type: 'personal', tier: 'free', billing_active: true, is_owner: true }
}

function setupOrgMock(overrides: Record<string, unknown> = {}) {
  mockInvoke.mockImplementation((cmd: unknown) => {
    if (typeof cmd !== 'string') return Promise.resolve(null)
    if (cmd in overrides) {
      const val = overrides[cmd]
      return val instanceof Error ? Promise.reject(val) : Promise.resolve(val)
    }
    if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true, post_wizard_completed: true }))
    if (cmd === 'get_license_signed_in') return Promise.resolve(true)
    if (cmd === 'list_projects') return Promise.resolve([makeProject()])
    if (cmd === 'get_all_drafts') return Promise.resolve([])
    if (cmd === 'list_connected_platforms') return Promise.resolve([])
    if (cmd === 'get_org_published') return Promise.resolve([])
    return Promise.resolve(null)
  })
}

beforeEach(() => { vi.clearAllMocks() })

describe('App routing — no-orgs and load-error states', () => {
  it('shows Create workspace button when list_projects returns empty array', async () => {
    setupOrgMock({ list_projects: [] })
    render(<App />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /create your first workspace/i })).toBeInTheDocument()
    )
    expect(screen.queryByRole('button', { name: /retry/i })).not.toBeInTheDocument()
  })

  it('shows error and Retry when list_projects rejects', async () => {
    setupOrgMock({ list_projects: new Error('Network timeout') })
    render(<App />)
    await waitFor(() => expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /create your first workspace/i })).not.toBeInTheDocument()
  })
})

describe('App routing — org navigation', () => {
  beforeEach(() => { setupOrgMock() })

  it('renders PostTable in org_queue after projects load', async () => {
    render(<App />)
    await waitFor(() => expect(screen.getByTestId('select-post')).toBeInTheDocument())
  })

  it('clicking post row shows EditPostView', async () => {
    render(<App />)
    await waitFor(() => expect(screen.getByTestId('select-post')).toBeInTheDocument())
    await userEvent.setup().click(screen.getByTestId('select-post'))
    await waitFor(() => expect(screen.getByTestId('edit-post-view')).toBeInTheDocument())
  })

  it('queue load error shows Retry', async () => {
    setupOrgMock({ get_all_drafts: new Error('server error') })
    render(<App />)
    await waitFor(() => expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument())
  })

  it('navigating to org_history renders OrgHistoryView', async () => {
    render(<App />)
    await waitFor(() => expect(screen.getByTestId('select-post')).toBeInTheDocument())
    await userEvent.setup().click(screen.getByTestId('nav-org-history'))
    await waitFor(() => expect(screen.getByTestId('select-post')).toBeInTheDocument())
  })

  it('navigating to org_settings renders OrgSettingsView', async () => {
    render(<App />)
    await waitFor(() => expect(screen.getByTestId('nav-org-settings')).toBeInTheDocument())
    await userEvent.setup().click(screen.getByTestId('nav-org-settings'))
    await waitFor(() => expect(screen.getByText('OrgSettingsView')).toBeInTheDocument())
  })

  it('leftnav settings opens AccountSettingsView', async () => {
    render(<App />)
    await waitFor(() => expect(screen.getByTestId('leftnav-settings')).toBeInTheDocument())
    await userEvent.setup().click(screen.getByTestId('leftnav-settings'))
    await waitFor(() => expect(screen.getByText('AccountSettingsView')).toBeInTheDocument())
  })
})

describe('App — dirty nav guard', () => {
  beforeEach(() => { setupOrgMock() })

  async function openDirtyEditView() {
    render(<App />)
    await waitFor(() => expect(screen.getByTestId('select-post')).toBeInTheDocument())
    await userEvent.setup().click(screen.getByTestId('select-post'))
    await waitFor(() => expect(screen.getByTestId('dirty-btn')).toBeInTheDocument())
    await userEvent.setup().click(screen.getByTestId('dirty-btn'))
  }

  it('shows discard modal when nav clicked with dirty editor', async () => {
    await openDirtyEditView()
    await userEvent.setup().click(screen.getByTestId('nav-org-settings'))
    await waitFor(() => expect(screen.getByText(/unsaved changes/i)).toBeInTheDocument())
  })

  it('Discard navigates away from EditPostView', async () => {
    await openDirtyEditView()
    await userEvent.setup().click(screen.getByTestId('nav-org-settings'))
    await waitFor(() => expect(screen.getByText(/unsaved changes/i)).toBeInTheDocument())
    await userEvent.setup().click(screen.getByRole('button', { name: /discard/i }))
    await waitFor(() => expect(screen.getByText('OrgSettingsView')).toBeInTheDocument())
    expect(screen.queryByTestId('edit-post-view')).not.toBeInTheDocument()
  })

  it('Cancel keeps user on EditPostView', async () => {
    await openDirtyEditView()
    await userEvent.setup().click(screen.getByTestId('nav-org-settings'))
    await waitFor(() => expect(screen.getByText(/unsaved changes/i)).toBeInTheDocument())
    await userEvent.setup().click(screen.getByRole('button', { name: /cancel/i }))
    expect(screen.getByTestId('edit-post-view')).toBeInTheDocument()
    expect(screen.queryByText(/unsaved changes/i)).not.toBeInTheDocument()
  })
})
