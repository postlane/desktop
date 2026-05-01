// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
import { invoke } from '@tauri-apps/api/core'

import ModalDone from './ModalDone'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => { vi.clearAllMocks() })

describe('ModalDone', () => {
  it('test_open_postlane_invokes_set_completed', async () => {
    mockInvoke.mockResolvedValue(undefined)
    const onComplete = vi.fn()
    render(<ModalDone onComplete={onComplete} />)
    fireEvent.click(screen.getByRole('button', { name: /open postlane/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_wizard_completed')
      expect(onComplete).toHaveBeenCalledOnce()
    })
  })

  it('test_no_back_button', () => {
    render(<ModalDone onComplete={vi.fn()} />)
    expect(screen.queryByRole('button', { name: /back/i })).not.toBeInTheDocument()
  })
})
