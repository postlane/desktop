// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))

import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'

import ModalPricingGate from './ModalPricingGate'

const mockInvoke = vi.mocked(invoke)
const mockOpenUrl = vi.mocked(openUrl)

beforeEach(() => {
  vi.clearAllMocks()
})

describe('ModalPricingGate', () => {
  it('test_subscribe_opens_billing_url', async () => {
    mockOpenUrl.mockResolvedValue(undefined)
    mockInvoke.mockResolvedValue('none')
    render(<ModalPricingGate onPaid={vi.fn()} onBack={vi.fn()} pollIntervalMs={50} maxAttempts={1} />)
    fireEvent.click(screen.getByRole('button', { name: /subscribe/i }))
    await waitFor(() => {
      expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/billing')
    })
  })

  it('test_polling_advances_on_paid', async () => {
    mockOpenUrl.mockResolvedValue(undefined)
    mockInvoke.mockResolvedValue('paid')
    const onPaid = vi.fn()
    render(<ModalPricingGate onPaid={onPaid} onBack={vi.fn()} pollIntervalMs={30} maxAttempts={5} />)
    fireEvent.click(screen.getByRole('button', { name: /subscribe/i }))
    await waitFor(() => {
      expect(onPaid).toHaveBeenCalledOnce()
    }, { timeout: 3000 })
  })

  it('test_check_again_button_after_timeout', async () => {
    mockOpenUrl.mockResolvedValue(undefined)
    mockInvoke.mockResolvedValue('none')
    render(<ModalPricingGate onPaid={vi.fn()} onBack={vi.fn()} pollIntervalMs={30} maxAttempts={2} />)
    fireEvent.click(screen.getByRole('button', { name: /subscribe/i }))
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /check again/i })).toBeInTheDocument()
    }, { timeout: 3000 })
  })
})
