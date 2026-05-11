// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import '@testing-library/jest-dom'
import PreviewModal, { CharCount, CHAR_LIMITS } from './PreviewModal'
import type { ImageState } from '../types'

describe('PreviewModal — platform badge', () => {
  it('shows the platform name as a badge', () => {
    render(
      <PreviewModal
        platform="x"
        text="Hello world"
        imageState={{ status: 'none' }}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByText(/\bx\b/i)).toBeInTheDocument()
  })

  it('shows linkedin badge for linkedin platform', () => {
    render(
      <PreviewModal
        platform="linkedin"
        text="Hello world"
        imageState={{ status: 'none' }}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByText(/linkedin/i)).toBeInTheDocument()
  })
})

describe('PreviewModal — close behaviour', () => {
  it('calls onClose when the close button is clicked', () => {
    const onClose = vi.fn()
    render(
      <PreviewModal
        platform="x"
        text="Hello"
        imageState={{ status: 'none' }}
        onClose={onClose}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /close/i }))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('calls onClose when the modal overlay is clicked', () => {
    const onClose = vi.fn()
    render(
      <PreviewModal
        platform="x"
        text="Hello"
        imageState={{ status: 'none' }}
        onClose={onClose}
      />,
    )
    fireEvent.click(screen.getByTestId('modal-overlay'))
    expect(onClose).toHaveBeenCalledOnce()
  })
})

describe('PreviewModal — OG image', () => {
  it('renders OG image when imageState is loaded', () => {
    const state: ImageState = { status: 'loaded', url: 'https://example.com/og.png' }
    render(
      <PreviewModal
        platform="x"
        text="Hello"
        imageState={state}
        onClose={vi.fn()}
      />,
    )
    const img = screen.getByRole('img')
    expect(img).toBeInTheDocument()
    expect(img).toHaveAttribute('src', 'https://example.com/og.png')
  })

  it('does not render an image when imageState is none', () => {
    render(
      <PreviewModal
        platform="x"
        text="Hello"
        imageState={{ status: 'none' }}
        onClose={vi.fn()}
      />,
    )
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })

  it('does not render an image when imageState is loading', () => {
    render(
      <PreviewModal
        platform="x"
        text="Hello"
        imageState={{ status: 'loading' }}
        onClose={vi.fn()}
      />,
    )
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })

  it('does not render an image when imageState is error', () => {
    render(
      <PreviewModal
        platform="x"
        text="Hello"
        imageState={{ status: 'error', message: 'Not found' }}
        onClose={vi.fn()}
      />,
    )
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })
})

describe('CharCount', () => {
  it('shows used / limit for x (280 limit)', () => {
    render(<CharCount platform="x" text={'a'.repeat(100)} />)
    expect(screen.getByText('100 / 280')).toBeInTheDocument()
  })

  it('shows used / limit when over limit', () => {
    render(<CharCount platform="x" text={'a'.repeat(290)} />)
    expect(screen.getByText('290 / 280')).toBeInTheDocument()
  })

  it('shows used / limit when exactly at limit', () => {
    render(<CharCount platform="x" text={'a'.repeat(280)} />)
    expect(screen.getByText('280 / 280')).toBeInTheDocument()
  })

  it('shows count in danger colour when over limit', () => {
    const { container } = render(<CharCount platform="x" text={'a'.repeat(290)} />)
    expect(container.firstChild).toHaveClass('has-text-danger')
  })

  it('CHAR_LIMITS exports x as 280', () => {
    expect(CHAR_LIMITS['x']).toBe(280)
  })

  it('CHAR_LIMITS exports linkedin', () => {
    expect(CHAR_LIMITS['linkedin']).toBeGreaterThan(0)
  })
})
