// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
import { invoke } from '@tauri-apps/api/core'

import ModalDescribeRepo from './ModalDescribeRepo'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => { vi.clearAllMocks() })

describe('ModalDescribeRepo', () => {
  it('test_prefills_from_remote_name', async () => {
    mockInvoke.mockResolvedValue('my-awesome-repo')
    render(<ModalDescribeRepo projectId="proj-1" repoPath="/some/repo" onNext={vi.fn()} onBack={vi.fn()} />)
    await waitFor(() => {
      expect(screen.getByRole('textbox')).toHaveValue('my-awesome-repo')
    })
  })

  it('test_next_disabled_when_empty', async () => {
    mockInvoke.mockResolvedValue(null)
    render(<ModalDescribeRepo projectId="proj-1" repoPath="/some/repo" onNext={vi.fn()} onBack={vi.fn()} />)
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /next/i })).toBeDisabled()
    })
  })

  it('test_next_registers_repo', async () => {
    const onNext = vi.fn()
    mockInvoke
      .mockResolvedValueOnce('remote-name')
      .mockResolvedValueOnce('Registered')
    render(<ModalDescribeRepo projectId="proj-1" repoPath="/some/repo" onNext={onNext} onBack={vi.fn()} />)
    await waitFor(() => {
      expect(screen.getByRole('textbox')).toHaveValue('remote-name')
    })
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('register_repo_with_project', {
        projectId: 'proj-1',
        repoPath: '/some/repo',
        description: 'remote-name',
      })
      expect(onNext).toHaveBeenCalledWith('remote-name')
    })
  })
})
