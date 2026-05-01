// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
import { invoke } from '@tauri-apps/api/core'

import ModalConnectScheduler from './ModalConnectScheduler'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => { vi.clearAllMocks() })

describe('ModalConnectScheduler', () => {
  it('test_next_disabled_before_connection_test', () => {
    render(<ModalConnectScheduler onNext={vi.fn()} onBack={vi.fn()} onSetupLater={vi.fn()} />)
    expect(screen.getByRole('button', { name: /^next$/i })).toBeDisabled()
  })

  it('test_next_enabled_after_success', async () => {
    mockInvoke.mockResolvedValue(undefined)
    render(<ModalConnectScheduler onNext={vi.fn()} onBack={vi.fn()} onSetupLater={vi.fn()} />)
    fireEvent.change(screen.getByPlaceholderText(/api key/i), { target: { value: 'key-123' } })
    fireEvent.click(screen.getByRole('button', { name: /test connection/i }))
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /^next$/i })).not.toBeDisabled()
    })
  })

  it('test_set_up_later_advances', () => {
    const onSetupLater = vi.fn()
    render(<ModalConnectScheduler onNext={vi.fn()} onBack={vi.fn()} onSetupLater={onSetupLater} />)
    fireEvent.click(screen.getByRole('button', { name: /set up later/i }))
    expect(onSetupLater).toHaveBeenCalledOnce()
  })
})
