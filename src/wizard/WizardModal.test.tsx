// SPDX-License-Identifier: BUSL-1.1

import React from 'react'
import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import '@testing-library/jest-dom'
import WizardModal from './WizardModal'

function renderModal(props: Partial<React.ComponentProps<typeof WizardModal>> = {}) {
  return render(
    <WizardModal
      title="Test Title"
      subtitle="Test subtitle."
      onNext={vi.fn()}
      {...props}
    >
      <div>Modal content</div>
    </WizardModal>,
  )
}

describe('WizardModal', () => {
  it('renders title and subtitle', () => {
    renderModal({ title: 'Welcome to Postlane', subtitle: 'Set up your workspace.' })
    expect(screen.getByText('Welcome to Postlane')).toBeInTheDocument()
    expect(screen.getByText('Set up your workspace.')).toBeInTheDocument()
  })

  it('renders children', () => {
    render(
      <WizardModal title="T" subtitle="S" onNext={vi.fn()}>
        <p>Inner content</p>
      </WizardModal>,
    )
    expect(screen.getByText('Inner content')).toBeInTheDocument()
  })

  it('hides Back button when onBack is not provided', () => {
    renderModal()
    expect(screen.queryByRole('button', { name: /back/i })).not.toBeInTheDocument()
  })

  it('shows Back button and calls onBack when provided', () => {
    const onBack = vi.fn()
    renderModal({ onBack })
    const btn = screen.getByRole('button', { name: /back/i })
    fireEvent.click(btn)
    expect(onBack).toHaveBeenCalledOnce()
  })

  it('hides Skip button when onSkip is not provided', () => {
    renderModal()
    expect(screen.queryByRole('button', { name: /skip/i })).not.toBeInTheDocument()
  })

  it('shows Skip button and calls onSkip when provided', () => {
    const onSkip = vi.fn()
    renderModal({ onSkip })
    const btn = screen.getByRole('button', { name: /skip/i })
    fireEvent.click(btn)
    expect(onSkip).toHaveBeenCalledOnce()
  })

  it('disables Next button when nextDisabled is true', () => {
    renderModal({ nextDisabled: true })
    expect(screen.getByRole('button', { name: /next/i })).toBeDisabled()
  })

  it('uses nextLabel as the Next button label', () => {
    renderModal({ nextLabel: 'Get started' })
    expect(screen.getByRole('button', { name: /get started/i })).toBeInTheDocument()
  })

  it('calls onNext when Next is clicked', () => {
    const onNext = vi.fn()
    renderModal({ onNext })
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    expect(onNext).toHaveBeenCalledOnce()
  })
})
