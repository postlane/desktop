// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('../wizard/AddRepoModal', () => ({
  default: ({ onClose }: { onClose: () => void }) => (
    <div data-testid="add-repo-modal"><button onClick={onClose}>Close modal</button></div>
  ),
}))
vi.mock('./WorkspaceConfirmModal', () => ({
  default: () => <div data-testid="workspace-confirm-modal" />,
}))
vi.mock('./VoiceGuideHint', () => ({
  default: ({ workspacePath, onDismiss }: { workspacePath: string; onDismiss?: () => void }) => (
    <div data-testid="voice-guide-hint" data-workspace-path={workspacePath}>
      {onDismiss && <button onClick={onDismiss}>Dismiss hint</button>}
    </div>
  ),
}))

import { invoke } from '../ipc/invoke'
import RepositoriesBlock from './RepositoriesBlock'

const mockInvoke = vi.mocked(invoke)

function makeRow() {
  return {
    repo_id: 'repo-1', github_full_name: null, local_path: '/repos/myrepo',
    display_name: 'MyRepo', github_app_connected: false, folder_registered: true,
    cli_initialized: true, project_id_mismatch: false,
  }
}

beforeEach(() => { vi.clearAllMocks() })

// ── M5: voice guide hint dismiss persistence ──────────────────────────────────

describe('M5: voice guide hint dismiss persists across launches', () => {
  it('does not show hint when voice_guide_hint_dismissed is true in app state', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'get_workspace_path') return '/Users/hugo/code/myorg'
      if (cmd === 'get_app_state') return { voice_guide_hint_dismissed: true }
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" isOwner={true} />)
    // wait until get_app_state has been called (effects settled), THEN assert no hint
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_app_state'))
    expect(screen.queryByTestId('voice-guide-hint')).not.toBeInTheDocument()
  })

  it('shows hint when voice_guide_hint_dismissed is false in app state', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'get_workspace_path') return '/Users/hugo/code/myorg'
      if (cmd === 'get_app_state') return { voice_guide_hint_dismissed: false }
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByTestId('voice-guide-hint')).toBeInTheDocument())
  })

  it('calls save_app_state_command with voice_guide_hint_dismissed:true on dismiss', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'get_workspace_path') return '/Users/hugo/code/myorg'
      if (cmd === 'get_app_state') return { voice_guide_hint_dismissed: false }
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByTestId('voice-guide-hint')).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Dismiss hint/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_app_state_command', {
        state: expect.objectContaining({ voice_guide_hint_dismissed: true }),
      })
    )
  })

  it('hides hint immediately after dismissal without page reload', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'get_workspace_path') return '/Users/hugo/code/myorg'
      if (cmd === 'get_app_state') return { voice_guide_hint_dismissed: false }
      return null
    })
    render(<RepositoriesBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByTestId('voice-guide-hint')).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Dismiss hint/i }))
    await waitFor(() => expect(screen.queryByTestId('voice-guide-hint')).not.toBeInTheDocument())
  })
})
