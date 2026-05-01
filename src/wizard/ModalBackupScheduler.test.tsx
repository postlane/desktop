// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
import { invoke } from '@tauri-apps/api/core'

import ModalBackupScheduler from './ModalBackupScheduler'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => { vi.clearAllMocks() })

describe('ModalBackupScheduler', () => {
  it('test_skip_advances', async () => {
    mockInvoke.mockResolvedValue(false)
    const onSkip = vi.fn()
    render(<ModalBackupScheduler onNext={vi.fn()} onBack={vi.fn()} onSkip={onSkip} />)
    await waitFor(() => {
      const skipBtn = screen.getByRole('button', { name: /skip/i })
      skipBtn.click()
    })
    expect(onSkip).toHaveBeenCalledOnce()
  })

  it('test_shows_configured_providers', async () => {
    mockInvoke.mockResolvedValue(true)
    render(<ModalBackupScheduler onNext={vi.fn()} onBack={vi.fn()} onSkip={vi.fn()} />)
    await waitFor(() => {
      expect(screen.getByText(/zernio/i)).toBeInTheDocument()
    })
  })
})
