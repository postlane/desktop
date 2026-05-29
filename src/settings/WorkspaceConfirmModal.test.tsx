// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { invoke } from '../ipc/invoke'
import WorkspaceConfirmModal from './WorkspaceConfirmModal'
import type { WorkspaceSetupResult } from './WorkspaceConfirmModal'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => vi.clearAllMocks())

function makeResult(repoCount = 2): WorkspaceSetupResult {
  const repos = Array.from({ length: repoCount }, (_, i) => ({
    name: `repo-${i + 1}`,
    path: `/code/org/repo-${i + 1}`,
    posts_dir: `repo-${i + 1}`,
  }))
  return {
    workspace_id: 'ws-proj-1',
    workspace_path: '/code/org',
    discovered_repos: repos,
  }
}

// ── 22.3.3 — Confirmation step ────────────────────────────────────────────────

describe('WorkspaceConfirmModal — repo list (22.3.3)', () => {
  it('renders all discovered repos as checked checkboxes', () => {
    render(<WorkspaceConfirmModal result={makeResult(2)} onConfirm={vi.fn()} onCancel={vi.fn()} />)
    const checkboxes = screen.getAllByRole('checkbox')
    expect(checkboxes).toHaveLength(2)
    checkboxes.forEach((cb) => expect(cb).toBeChecked())
  })

  it('shows the repo name for each entry', () => {
    render(<WorkspaceConfirmModal result={makeResult(2)} onConfirm={vi.fn()} onCancel={vi.fn()} />)
    expect(screen.getByText('repo-1')).toBeInTheDocument()
    expect(screen.getByText('repo-2')).toBeInTheDocument()
  })

  it('Confirm button is enabled when at least one repo is checked', () => {
    render(<WorkspaceConfirmModal result={makeResult(2)} onConfirm={vi.fn()} onCancel={vi.fn()} />)
    expect(screen.getByRole('button', { name: /^Confirm$/i })).toBeEnabled()
  })

  it('Confirm button is disabled when all repos are unchecked', async () => {
    render(<WorkspaceConfirmModal result={makeResult(2)} onConfirm={vi.fn()} onCancel={vi.fn()} />)
    const checkboxes = screen.getAllByRole('checkbox')
    checkboxes.forEach((cb) => fireEvent.click(cb))
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /^Confirm$/i })).toBeDisabled()
    )
  })

  it('allows a repo to be deselected', async () => {
    render(<WorkspaceConfirmModal result={makeResult(2)} onConfirm={vi.fn()} onCancel={vi.fn()} />)
    const checkboxes = screen.getAllByRole('checkbox')
    fireEvent.click(checkboxes[0])
    await waitFor(() => expect(checkboxes[0]).not.toBeChecked())
    expect(checkboxes[1]).toBeChecked()
  })

  it('calls onCancel when Cancel is clicked', () => {
    const onCancel = vi.fn()
    render(<WorkspaceConfirmModal result={makeResult(2)} onConfirm={onCancel} onCancel={onCancel} />)
    fireEvent.click(screen.getByRole('button', { name: /^Cancel$/i }))
    expect(onCancel).toHaveBeenCalled()
  })
})

// ── 22.3.4 — Confirm calls confirm_workspace_repos ────────────────────────────

describe('WorkspaceConfirmModal — confirm writes selected repos (22.3.4)', () => {
  it('calls confirm_workspace_repos with all paths when all checked', async () => {
    mockInvoke.mockResolvedValue(null)
    const onConfirm = vi.fn()
    render(<WorkspaceConfirmModal result={makeResult(2)} onConfirm={onConfirm} onCancel={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /^Confirm$/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('confirm_workspace_repos', {
        workspaceId: 'ws-proj-1',
        selectedPaths: ['/code/org/repo-1', '/code/org/repo-2'],
      })
    )
  })

  it('calls confirm_workspace_repos with only selected paths when one deselected', async () => {
    mockInvoke.mockResolvedValue(null)
    const onConfirm = vi.fn()
    render(<WorkspaceConfirmModal result={makeResult(2)} onConfirm={onConfirm} onCancel={vi.fn()} />)
    const checkboxes = screen.getAllByRole('checkbox')
    fireEvent.click(checkboxes[0]) // deselect repo-1
    await waitFor(() => expect(checkboxes[0]).not.toBeChecked())
    fireEvent.click(screen.getByRole('button', { name: /^Confirm$/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('confirm_workspace_repos', {
        workspaceId: 'ws-proj-1',
        selectedPaths: ['/code/org/repo-2'],
      })
    )
  })

  it('calls onConfirm callback after confirm_workspace_repos succeeds', async () => {
    mockInvoke.mockResolvedValue(null)
    const onConfirm = vi.fn()
    render(<WorkspaceConfirmModal result={makeResult(2)} onConfirm={onConfirm} onCancel={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /^Confirm$/i }))
    await waitFor(() => expect(onConfirm).toHaveBeenCalled())
  })

  it('shows an error when confirm_workspace_repos fails', async () => {
    mockInvoke.mockRejectedValue('PL-WS-009: unexpected error')
    render(<WorkspaceConfirmModal result={makeResult(2)} onConfirm={vi.fn()} onCancel={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /^Confirm$/i }))
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
  })
})
