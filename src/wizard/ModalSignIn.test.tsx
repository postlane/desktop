// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))

import { invoke } from '@tauri-apps/api/core'

import ModalSignIn from './ModalSignIn'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => { vi.clearAllMocks() })

describe('ModalSignIn', () => {
  it('test_next_disabled_before_token', () => {
    mockInvoke.mockResolvedValue(false)
    render(<ModalSignIn onNext={vi.fn()} onTokenDetected={vi.fn()} pollIntervalMs={30} />)
    expect(screen.getByRole('button', { name: /next/i })).toBeDisabled()
  })

  it('test_next_enabled_after_token', async () => {
    mockInvoke.mockResolvedValue(true)
    render(<ModalSignIn onNext={vi.fn()} onTokenDetected={vi.fn()} pollIntervalMs={30} />)
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /next/i })).not.toBeDisabled()
    }, { timeout: 3000 })
  })

  it('test_apple_button_is_disabled', () => {
    mockInvoke.mockResolvedValue(false)
    render(<ModalSignIn onNext={vi.fn()} onTokenDetected={vi.fn()} pollIntervalMs={30} />)
    expect(screen.getByRole('button', { name: /apple/i })).toBeDisabled()
  })
})
