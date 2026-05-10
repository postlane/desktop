// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { invoke } from '../ipc/invoke'
import MembersBlock from './MembersBlock'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => { vi.clearAllMocks() })

describe('MembersBlock', () => {
  it('renders without crashing', () => {
    render(<MembersBlock />)
  })

  it('shows placeholder text', () => {
    render(<MembersBlock />)
    expect(screen.getByText(/Member management coming soon/i)).toBeInTheDocument()
  })

  it('makes no invoke call on mount', () => {
    render(<MembersBlock />)
    expect(mockInvoke).not.toHaveBeenCalled()
  })
})
