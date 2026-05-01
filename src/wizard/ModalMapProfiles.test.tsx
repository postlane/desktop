// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import '@testing-library/jest-dom'

beforeEach(() => { vi.clearAllMocks() })

import ModalMapProfiles from './ModalMapProfiles'

describe('ModalMapProfiles', () => {
  it('test_skip_advances', () => {
    const onSkip = vi.fn()
    render(<ModalMapProfiles onNext={vi.fn()} onBack={vi.fn()} onSkip={onSkip} />)
    fireEvent.click(screen.getByRole('button', { name: /skip/i }))
    expect(onSkip).toHaveBeenCalledOnce()
  })

  it('test_renders_platform_dropdowns', () => {
    render(<ModalMapProfiles onNext={vi.fn()} onBack={vi.fn()} onSkip={vi.fn()} />)
    expect(screen.getByLabelText(/x \(twitter\)/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/bluesky/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/linkedin/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/mastodon/i)).toBeInTheDocument()
  })
})
