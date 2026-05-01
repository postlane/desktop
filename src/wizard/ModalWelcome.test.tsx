// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))
import { openUrl } from '@tauri-apps/plugin-opener'

import ModalWelcome from './ModalWelcome'

const mockOpenUrl = vi.mocked(openUrl)

beforeEach(() => { vi.clearAllMocks() })

describe('ModalWelcome', () => {
  it('test_get_started_calls_next', () => {
    const onNext = vi.fn()
    render(<ModalWelcome onNext={onNext} />)
    fireEvent.click(screen.getByRole('button', { name: /get started/i }))
    expect(onNext).toHaveBeenCalledOnce()
  })

  it('test_pricing_link_opens_browser', () => {
    render(<ModalWelcome onNext={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /see pricing/i }))
    expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/pricing')
  })
})
