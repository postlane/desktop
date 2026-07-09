// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('../wizard/workspace-setup/WorkspaceSetupWizard', () => ({
  default: ({ projectId, projectName, onComplete, onBack }: {
    projectId: string; projectName: string; onComplete: () => void; onBack: () => void;
  }) => (
    <div data-testid="workspace-setup-wizard" data-project-id={projectId} data-project-name={projectName}>
      <button onClick={onComplete}>complete-setup-wizard</button>
      <button onClick={onBack}>close-setup-wizard</button>
    </div>
  ),
}))
vi.mock('./VoiceGuideHint', () => ({
  default: ({ workspacePath, onDismiss }: { workspacePath: string; onDismiss: () => void }) => (
    <div data-testid="voice-guide-hint" data-workspace-path={workspacePath}>
      <button onClick={onDismiss}>Dismiss hint</button>
    </div>
  ),
}))

import { invoke } from '../ipc/invoke'
import RepositoriesBlock from './RepositoriesBlock'

const mockInvoke = vi.mocked(invoke)

// ── Helpers ────────────────────────────────────────────────────────────────────

interface RepoConnectionStatus {
  repo_id: string | null; github_full_name: string | null; local_path: string | null;
  display_name: string; github_app_connected: boolean; folder_registered: boolean;
  cli_initialized: boolean; project_id_mismatch: boolean;
}

function makeRow(overrides: Partial<RepoConnectionStatus> = {}): RepoConnectionStatus {
  return {
    repo_id: 'repo-1', github_full_name: null, local_path: '/repos/myrepo',
    display_name: 'MyRepo', github_app_connected: false, folder_registered: true,
    cli_initialized: true, project_id_mismatch: false,
    ...overrides,
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'get_repo_connection_status') return [makeRow()]
    return null
  })
})

// ── 22.3.1 — Add workspace CTA ────────────────────────────────────────────────

describe('RepositoriesBlock — Add workspace CTA (22.3.1)', () => {
  it('shows "Add workspace" as primary CTA for owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="North Lane" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /^Add workspace$/i })).toBeInTheDocument()
    )
  })

  it('does not show "Add individual repository" (removed in favour of workspace flow)', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="North Lane" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('button', { name: /Add workspace/i })).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Add individual repository/i })).not.toBeInTheDocument()
  })

  it('does not show a button labeled "Add repository" alone', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="North Lane" isOwner={true} />)
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: /^Add repository$/i })).not.toBeInTheDocument()
    )
  })
})

// ── checklist 24.3.7 — repointed to WorkspaceSetupWizard ─────────────────────

describe('RepositoriesBlock — Add workspace opens WorkspaceSetupWizard (checklist 24.3.7)', () => {
  it('opens WorkspaceSetupWizard with the correct projectId/projectName when "Add workspace" is clicked', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="North Lane" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    const wizard = screen.getByTestId('workspace-setup-wizard')
    expect(wizard.dataset.projectId).toBe('proj-1')
    expect(wizard.dataset.projectName).toBe('North Lane')
  })

  it('closes the wizard modal when its onBack fires', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="North Lane" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    fireEvent.click(screen.getByText('close-setup-wizard'))
    expect(screen.queryByTestId('workspace-setup-wizard')).not.toBeInTheDocument()
  })

  it('closes the wizard modal and refreshes the repo list when its onComplete fires', async () => {
    render(<RepositoriesBlock projectId="proj-1" projectName="North Lane" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /^Add workspace$/i })))
    const callsBefore = mockInvoke.mock.calls.filter((c) => c[0] === 'get_repo_connection_status').length
    fireEvent.click(screen.getByText('complete-setup-wizard'))
    expect(screen.queryByTestId('workspace-setup-wizard')).not.toBeInTheDocument()
    await waitFor(() => {
      const callsAfter = mockInvoke.mock.calls.filter((c) => c[0] === 'get_repo_connection_status').length
      expect(callsAfter).toBeGreaterThan(callsBefore)
    })
  })
})

// ── 22.3.22a: permanent voice guide hint in settings ─────────────────────────

describe('22.3.22a: permanent voice guide hint', () => {
  it('shows voice guide hint when get_workspace_path returns a path', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'get_workspace_path') return '/Users/hugo/code/myorg'
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="North Lane" isOwner={true} />)
    await waitFor(() => {
      const hint = screen.getByTestId('voice-guide-hint')
      expect(hint).toBeInTheDocument()
      expect(hint.getAttribute('data-workspace-path')).toBe('/Users/hugo/code/myorg')
    })
  })

  it('does not show voice guide hint when no workspace path returned', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'get_workspace_path') return null
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" projectName="North Lane" isOwner={true} />)
    await waitFor(() => expect(screen.queryByTestId('voice-guide-hint')).not.toBeInTheDocument())
  })
})
