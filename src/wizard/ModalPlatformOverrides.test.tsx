// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
import { invoke } from '@tauri-apps/api/core'

import ModalPlatformOverrides from './ModalPlatformOverrides'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => { vi.clearAllMocks() })

describe('ModalPlatformOverrides', () => {
  it('test_skip_preserves_defaults', async () => {
    mockInvoke.mockResolvedValue(undefined)
    const onSkip = vi.fn()
    render(<ModalPlatformOverrides repoPath="/some/repo" onNext={vi.fn()} onBack={vi.fn()} onSkip={onSkip} />)
    fireEvent.click(screen.getByRole('button', { name: /skip/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_repo_platform_overrides', {
        repoPath: '/some/repo',
        overrides: { x: true, bluesky: true, linkedin: true, mastodon: true },
      })
      expect(onSkip).toHaveBeenCalledOnce()
    })
  })

  it('test_toggle_off_persists', async () => {
    mockInvoke.mockResolvedValue(undefined)
    const onNext = vi.fn()
    render(<ModalPlatformOverrides repoPath="/some/repo" onNext={onNext} onBack={vi.fn()} onSkip={vi.fn()} />)
    fireEvent.click(screen.getByRole('switch', { name: /bluesky/i }))
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_repo_platform_overrides', {
        repoPath: '/some/repo',
        overrides: { x: true, bluesky: false, linkedin: true, mastodon: true },
      })
      expect(onNext).toHaveBeenCalledOnce()
    })
  })
})
