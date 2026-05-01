// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
import { invoke } from '@tauri-apps/api/core'

import ModalNameWorkspace from './ModalNameWorkspace'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => { vi.clearAllMocks() })

describe('ModalNameWorkspace', () => {
  it('test_next_disabled_when_empty', () => {
    render(<ModalNameWorkspace onNext={vi.fn()} onBack={vi.fn()} />)
    expect(screen.getByRole('button', { name: /next/i })).toBeDisabled()
  })

  it('test_stores_project_id_on_success', async () => {
    const onNext = vi.fn()
    mockInvoke.mockResolvedValue({ project_id: 'proj-abc', name: 'Acme', workspace_type: 'personal' })
    render(<ModalNameWorkspace onNext={onNext} onBack={vi.fn()} />)
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Acme' } })
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('create_project', { name: 'Acme', workspaceType: 'personal' })
      expect(onNext).toHaveBeenCalledWith('proj-abc')
    })
  })

  it('test_shows_error_on_402', async () => {
    mockInvoke.mockRejectedValue(new Error('no_free_slot'))
    render(<ModalNameWorkspace onNext={vi.fn()} onBack={vi.fn()} />)
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Acme' } })
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument()
    })
  })

  it('test_renders_workspace_type_dropdown', () => {
    render(<ModalNameWorkspace onNext={vi.fn()} onBack={vi.fn()} />)
    const select = screen.getByRole('combobox')
    expect(select).toBeInTheDocument()
    expect(screen.getByRole('option', { name: 'Personal' })).toBeInTheDocument()
    expect(screen.getByRole('option', { name: 'Organization' })).toBeInTheDocument()
    expect(screen.getByRole('option', { name: 'Client project' })).toBeInTheDocument()
  })

  it('test_passes_workspace_type_to_invoke', async () => {
    const onNext = vi.fn()
    mockInvoke.mockResolvedValue({ project_id: 'proj-xyz', name: 'Acme', workspace_type: 'organization' })
    render(<ModalNameWorkspace onNext={onNext} onBack={vi.fn()} />)
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Acme' } })
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'organization' } })
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('create_project', { name: 'Acme', workspaceType: 'organization' })
      expect(onNext).toHaveBeenCalledWith('proj-xyz')
    })
  })
})
