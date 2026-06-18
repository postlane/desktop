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
  default: ({ workspacePath }: { workspacePath: string }) => (
    <div data-testid="voice-guide-hint" data-workspace-path={workspacePath} />
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

// ── 22.3.17 — Rescan workspace button ─────────────────────────────────────────

describe('RepositoriesBlock — Rescan workspace button visibility (22.3.17)', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      return null
    })
  })

  it('shows "Rescan workspace" button for owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('button', { name: /Rescan workspace/i })).toBeInTheDocument())
  })

  it('hides "Rescan workspace" button for non-owners', async () => {
    render(<RepositoriesBlock projectId="proj-1" isOwner={false} />)
    await waitFor(() => expect(screen.getByText('MyRepo')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /Rescan workspace/i })).not.toBeInTheDocument()
  })
})

// ── 22.3.20 — Rescan workspace results ────────────────────────────────────────

describe('RepositoriesBlock — Rescan workspace results (22.3.20)', () => {
  function setupRescan(result: object | Error) {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_repo_connection_status') return [makeRow()]
      if (cmd === 'rescan_workspace') {
        if (result instanceof Error) throw result
        return result
      }
      return null
    })
  }
  const noChange = { added: [], deactivated: [], unchanged: ['MyRepo'] }

  it('calls rescan_workspace with the correct workspaceId', async () => {
    setupRescan(noChange)
    render(<RepositoriesBlock projectId="proj-42" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Rescan workspace/i })))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('rescan_workspace', { workspaceId: 'proj-42' })
    )
  })

  it('shows "All repos up to date." when rescan finds no changes', async () => {
    setupRescan(noChange)
    render(<RepositoriesBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Rescan workspace/i })))
    await waitFor(() => expect(screen.getByText(/All repos up to date/i)).toBeInTheDocument())
  })

  it('shows added count when rescan finds new repos', async () => {
    setupRescan({ added: ['new-repo'], deactivated: [], unchanged: ['MyRepo'] })
    render(<RepositoriesBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Rescan workspace/i })))
    await waitFor(() => expect(screen.getByText(/Added: 1/i)).toBeInTheDocument())
  })

  it('refreshes the repo list after rescan completes (22.10.12)', async () => {
    setupRescan({ added: ['new-repo'], deactivated: [], unchanged: ['MyRepo'] })
    render(<RepositoriesBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Rescan workspace/i })))
    await waitFor(() => expect(screen.getByText(/Added: 1/i)).toBeInTheDocument())
    const statusCalls = mockInvoke.mock.calls.filter(c => c[0] === 'get_repo_connection_status')
    expect(statusCalls.length).toBeGreaterThan(1)
  })

  it('shows deactivated count when repos go missing', async () => {
    setupRescan({ added: [], deactivated: ['gone-repo'], unchanged: [] })
    render(<RepositoriesBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Rescan workspace/i })))
    await waitFor(() => expect(screen.getByText(/No longer found: 1/i)).toBeInTheDocument())
  })

  it('shows error message when rescan_workspace throws (UX-C8)', async () => {
    setupRescan(new Error('PL-SCAN-001: workspace not found'))
    render(<RepositoriesBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => fireEvent.click(screen.getByRole('button', { name: /Rescan workspace/i })))
    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument()
      expect(screen.getByRole('alert').textContent).toContain('PL-SCAN-001')
    })
  })
})
