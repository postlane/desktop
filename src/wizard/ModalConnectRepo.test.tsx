// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))

import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

import ModalConnectRepo from './ModalConnectRepo'

const mockInvoke = vi.mocked(invoke)
const mockOpen = vi.mocked(open)

beforeEach(() => { vi.clearAllMocks() })

describe('ModalConnectRepo — error paths', () => {
  it('test_rejects_non_git_directory', async () => {
    mockOpen.mockResolvedValue('/some/path')
    mockInvoke.mockRejectedValue(new Error('not a git repository'))
    render(<ModalConnectRepo projectId="proj-1" onNext={vi.fn()} onBack={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /browse/i }))
    await waitFor(() => {
      expect(screen.getByText(/isn't a git repository/i)).toBeInTheDocument()
    })
  })

  it('test_rejects_missing_postlane_dir', async () => {
    mockOpen.mockResolvedValue('/some/path')
    mockInvoke.mockRejectedValue(new Error('config not found'))
    render(<ModalConnectRepo projectId="proj-1" onNext={vi.fn()} onBack={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /browse/i }))
    await waitFor(() => {
      expect(screen.getByText(/npx @postlane\/cli init/i)).toBeInTheDocument()
    })
  })
})

describe('ModalConnectRepo — detection logic', () => {
  it('test_silent_add_on_owned_project', async () => {
    const onSilentAdd = vi.fn()
    mockOpen.mockResolvedValue('/some/repo')
    mockInvoke
      .mockResolvedValueOnce('existing-proj-id')    // read_project_id_from_path
      .mockResolvedValueOnce('owned')               // check_project_status
      .mockResolvedValueOnce({ id: 'repo-1', name: 'myrepo', path: '/some/repo', active: true, added_at: '' }) // add_repo
    render(<ModalConnectRepo projectId="proj-1" onNext={vi.fn()} onBack={vi.fn()} onSilentAdd={onSilentAdd} />)
    fireEvent.click(screen.getByRole('button', { name: /browse/i }))
    await waitFor(() => { expect(onSilentAdd).toHaveBeenCalledOnce() })
  })

  it('test_pricing_gate_on_no_slot', async () => {
    const onPricingGate = vi.fn()
    mockOpen.mockResolvedValue('/some/repo')
    mockInvoke
      .mockResolvedValueOnce('existing-proj-id')    // read_project_id_from_path
      .mockResolvedValueOnce('not_found')           // check_project_status
      .mockResolvedValueOnce('none')                // check_billing_gate
    render(<ModalConnectRepo projectId="proj-1" onNext={vi.fn()} onBack={vi.fn()} onPricingGate={onPricingGate} />)
    fireEvent.click(screen.getByRole('button', { name: /browse/i }))
    await waitFor(() => { expect(onPricingGate).toHaveBeenCalledOnce() })
  })

  it('test_project_picker_on_no_project_id', async () => {
    mockOpen.mockResolvedValue('/some/repo')
    mockInvoke.mockResolvedValueOnce(null)          // read_project_id_from_path → no project_id
    render(<ModalConnectRepo projectId="proj-1" onNext={vi.fn()} onBack={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /browse/i }))
    await waitFor(() => {
      expect(screen.getByLabelText(/select workspace/i)).toBeInTheDocument()
    })
  })
})

describe('ModalConnectRepo — workspace picker flow', () => {
  async function openPickerState(onNext = vi.fn()) {
    mockOpen.mockResolvedValue('/some/repo')
    mockInvoke.mockResolvedValueOnce(null) // read_project_id_from_path → no project_id
    render(<ModalConnectRepo projectId="proj-1" onNext={onNext} onBack={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /browse/i }))
    await waitFor(() => {
      expect(screen.getByLabelText(/select workspace/i)).toBeInTheDocument()
    })
    return onNext
  }

  it('test_picker_next_current_workspace_calls_add_repo_and_write_config', async () => {
    const onNext = await openPickerState()
    mockInvoke
      .mockResolvedValueOnce({ id: 'r', name: 'n', path: '/some/repo', active: true, added_at: '' })
      .mockResolvedValueOnce('Committed')
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    await waitFor(() => { expect(onNext).toHaveBeenCalledWith('/some/repo') })
    expect(mockInvoke).toHaveBeenCalledWith('add_repo', { path: '/some/repo' })
    expect(mockInvoke).toHaveBeenCalledWith('write_project_id_to_config', { repoPath: '/some/repo', projectId: 'proj-1' })
  })

  it('test_picker_new_workspace_shows_name_input', async () => {
    await openPickerState()
    fireEvent.change(screen.getByLabelText(/select workspace/i), { target: { value: 'new' } })
    await waitFor(() => {
      expect(screen.getByLabelText(/workspace name/i)).toBeInTheDocument()
    })
  })

  it('test_picker_next_new_workspace_calls_create_project_then_registers_repo', async () => {
    const onNext = await openPickerState()
    fireEvent.change(screen.getByLabelText(/select workspace/i), { target: { value: 'new' } })
    await waitFor(() => { expect(screen.getByLabelText(/workspace name/i)).toBeInTheDocument() })
    fireEvent.change(screen.getByLabelText(/workspace name/i), { target: { value: 'My WS' } })
    mockInvoke
      .mockResolvedValueOnce({ project_id: 'new-proj', name: 'My WS', workspace_type: 'personal' })
      .mockResolvedValueOnce({ id: 'r', name: 'n', path: '/some/repo', active: true, added_at: '' })
      .mockResolvedValueOnce('Committed')
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    await waitFor(() => { expect(onNext).toHaveBeenCalledWith('/some/repo') })
    expect(mockInvoke).toHaveBeenCalledWith('create_project', { name: 'My WS', workspaceType: 'personal' })
    expect(mockInvoke).toHaveBeenCalledWith('write_project_id_to_config', { repoPath: '/some/repo', projectId: 'new-proj' })
  })

  it('test_picker_next_disabled_when_new_workspace_name_empty', async () => {
    await openPickerState()
    fireEvent.change(screen.getByLabelText(/select workspace/i), { target: { value: 'new' } })
    await waitFor(() => { expect(screen.getByLabelText(/workspace name/i)).toBeInTheDocument() })
    expect(screen.getByRole('button', { name: /next/i })).toBeDisabled()
  })
})

describe('ModalConnectRepo — normal flow', () => {
  it('test_shows_commit_notice_before_advance', async () => {
    mockOpen.mockResolvedValue('/some/repo')
    mockInvoke
      .mockResolvedValueOnce('existing-proj-id')  // read_project_id_from_path
      .mockResolvedValueOnce('offline')           // check_project_status → fall through to normal flow
      .mockResolvedValueOnce({ id: 'repo-1', name: 'myrepo', path: '/some/repo', active: true, added_at: '' }) // add_repo
      .mockResolvedValueOnce('Committed project id to config')  // write_project_id_to_config
    render(<ModalConnectRepo projectId="proj-1" onNext={vi.fn()} onBack={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /browse/i }))
    await waitFor(() => {
      expect(screen.getByText(/committed/i)).toBeInTheDocument()
    })
    expect(screen.getByRole('button', { name: /next/i })).not.toBeDisabled()
  })
})
