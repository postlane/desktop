// SPDX-License-Identifier: BUSL-1.1
// checklist 24.4.13 — EU Article 11a withdrawal button, design brief 10.
// Flow (matching the Claude Design prototype's stronger reading of "no
// single click can complete withdrawal"): Statement (read-only, Continue)
// -> Confirm (separate view, acknowledgment checkbox gates Confirm
// withdrawal, Back returns to Statement) -> Confirming -> Success/Error.

import type { ComponentProps } from 'react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import { MantineProvider } from '@mantine/core'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { invoke } from '../ipc/invoke'
import WithdrawFromContractButton from './WithdrawFromContractButton'

const mockInvoke = vi.mocked(invoke)

function renderButton(props: Partial<ComponentProps<typeof WithdrawFromContractButton>> = {}) {
  return render(
    <MantineProvider>
      <WithdrawFromContractButton
        projectId="proj-1"
        workspaceName="acme-blog"
        billingActive={true}
        {...props}
      />
    </MantineProvider>,
  )
}

// Mantine's Modal mounts its content asynchronously (an internal Transition
// tick, even with jsdom's matchMedia/ResizeObserver polyfilled) -- opening
// it always needs a waitFor, not a synchronous assertion right after the
// triggering click.
async function openStatement() {
  fireEvent.click(screen.getByRole('button', { name: 'Withdraw from contract' }))
  await waitFor(() => expect(screen.getByRole('button', { name: 'Continue' })).toBeInTheDocument())
}

async function goToConfirm() {
  await openStatement()
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
  await waitFor(() => expect(screen.getByRole('checkbox')).toBeInTheDocument())
}

async function acknowledgeAndConfirm() {
  await goToConfirm()
  fireEvent.click(screen.getByRole('checkbox'))
  fireEvent.click(screen.getByRole('button', { name: 'Confirm withdrawal' }))
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe('WithdrawFromContractButton — label, visibility', () => {
  it('test_withdrawal_button_present_and_labeled: renders a button labelled exactly "Withdraw from contract"', () => {
    renderButton()
    expect(screen.getByRole('button', { name: 'Withdraw from contract' })).toBeInTheDocument()
  })

  it('is disabled with an explanatory title when the workspace is not on an active plan', () => {
    renderButton({ billingActive: false })
    const button = screen.getByRole('button', { name: 'Withdraw from contract' })
    expect(button).toBeDisabled()
    expect(button.getAttribute('title')).toMatch(/isn't on an active plan/i)
  })
})

describe('WithdrawFromContractButton — two-step confirm', () => {
  it('test_withdrawal_two_step_confirm_required: clicking the button opens the statement step without calling invoke', async () => {
    renderButton()
    await openStatement()
    expect(screen.getByText(/acme-blog/)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Confirm withdrawal' })).not.toBeInTheDocument()
    expect(mockInvoke).not.toHaveBeenCalled()
  })

  it('Continue moves to the confirm step, still without calling invoke', async () => {
    renderButton()
    await goToConfirm()
    expect(screen.getByRole('button', { name: 'Confirm withdrawal' })).toBeInTheDocument()
    expect(mockInvoke).not.toHaveBeenCalled()
  })

  it('Confirm withdrawal is disabled until the acknowledgment checkbox is checked', async () => {
    renderButton()
    await goToConfirm()
    expect(screen.getByRole('button', { name: 'Confirm withdrawal' })).toBeDisabled()
    fireEvent.click(screen.getByRole('checkbox'))
    expect(screen.getByRole('button', { name: 'Confirm withdrawal' })).not.toBeDisabled()
  })

  it('test_withdrawal_two_step_confirm_required: only calls invoke once the checkbox is checked and Confirm withdrawal is clicked', async () => {
    mockInvoke.mockResolvedValue({ status: 'inactive', refunded: false })
    renderButton()
    await acknowledgeAndConfirm()
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('withdraw_from_contract', { projectId: 'proj-1' }))
  })

  it('Back from the confirm step returns to the statement step', async () => {
    renderButton()
    await goToConfirm()
    fireEvent.click(screen.getByRole('button', { name: 'Back' }))
    await waitFor(() => expect(screen.getByRole('button', { name: 'Continue' })).toBeInTheDocument())
    expect(mockInvoke).not.toHaveBeenCalled()
  })

  it('Cancel in the statement step closes without calling invoke', async () => {
    renderButton()
    await openStatement()
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(mockInvoke).not.toHaveBeenCalled()
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Continue' })).not.toBeInTheDocument())
  })
})

describe('WithdrawFromContractButton — outcome states', () => {
  it('test_withdrawal_sends_confirmation_email: success state tells the user to check their email for a receipt', async () => {
    mockInvoke.mockResolvedValue({ status: 'inactive', refunded: false })
    renderButton()
    await acknowledgeAndConfirm()
    await waitFor(() => expect(screen.getByText(/check your email for a receipt/i)).toBeInTheDocument())
  })

  it('success state mentions the refund amount when a refund was issued', async () => {
    mockInvoke.mockResolvedValue({ status: 'inactive', refunded: true, refund_amount: 300 })
    renderButton()
    await acknowledgeAndConfirm()
    await waitFor(() => expect(screen.getByText(/3\.00/)).toBeInTheDocument())
  })

  it('success state does not mention a refund when none was issued', async () => {
    mockInvoke.mockResolvedValue({ status: 'inactive', refunded: false })
    renderButton()
    await acknowledgeAndConfirm()
    await waitFor(() => expect(screen.getByText(/check your email for a receipt/i)).toBeInTheDocument())
    expect(screen.queryByText(/refund/i)).not.toBeInTheDocument()
  })

  it('shows an error and keeps a retry action available when the request fails', async () => {
    mockInvoke.mockRejectedValue(new Error('unavailable'))
    renderButton()
    await acknowledgeAndConfirm()
    await waitFor(() => expect(screen.getByText(/unavailable/i)).toBeInTheDocument())
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument()
  })

  it('retry after an error calls invoke again', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('unavailable'))
    mockInvoke.mockResolvedValueOnce({ status: 'inactive', refunded: false })
    renderButton()
    await acknowledgeAndConfirm()
    await waitFor(() => expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(2))
  })
})
