// SPDX-License-Identifier: BUSL-1.1
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))

import { invoke } from '@tauri-apps/api/core'
import SignInScreen from './SignInScreen'

const mockInvoke = vi.mocked(invoke)
beforeEach(() => { vi.clearAllMocks() })

describe('SignInScreen', () => {
  it('renders sign-in heading', () => {
    mockInvoke.mockResolvedValue(false)
    render(<SignInScreen onSignedIn={vi.fn()} pollIntervalMs={10000} />)
    expect(screen.getByRole('heading', { name: /sign in/i })).toBeInTheDocument()
  })

  it('calls onSignedIn when token is detected', async () => {
    const onSignedIn = vi.fn()
    mockInvoke.mockResolvedValue(true)
    render(<SignInScreen onSignedIn={onSignedIn} pollIntervalMs={50} />)
    await waitFor(() => { expect(onSignedIn).toHaveBeenCalledOnce() })
  })

  it('shows GitHub sign-in button', () => {
    mockInvoke.mockResolvedValue(false)
    render(<SignInScreen onSignedIn={vi.fn()} pollIntervalMs={10000} />)
    expect(screen.getByRole('button', { name: /github/i })).toBeInTheDocument()
  })
})
