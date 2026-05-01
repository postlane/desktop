// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('./useWizardState', () => ({ useWizardState: vi.fn() }))

import { invoke } from '@tauri-apps/api/core'
import { useWizardState } from './useWizardState'
import type { WizardModalName } from './useWizardState'
import Wizard from './Wizard'

const mockInvoke = vi.mocked(invoke)
const mockUseWizardState = vi.mocked(useWizardState)

function makeState(modal: WizardModalName) {
  return {
    modal,
    projectId: null,
    selectedRepoPath: null,
    repoDescription: '',
    voiceGuide: '',
    schedulerConnected: false,
    licenseTokenPresent: false,
  }
}

function makeActions(overrides: Record<string, unknown> = {}) {
  return {
    next: vi.fn(),
    back: vi.fn(),
    skip: vi.fn(),
    startFirstLaunch: vi.fn(),
    startAddProject: vi.fn(),
    startAddRepo: vi.fn(),
    setLicenseTokenPresent: vi.fn(),
    setSchedulerConnected: vi.fn(),
    setProjectId: vi.fn(),
    setSelectedRepoPath: vi.fn(),
    setRepoDescription: vi.fn(),
    setVoiceGuide: vi.fn(),
    advanceScheduler: vi.fn(),
    advancePastGate: vi.fn(),
    jumpToDone: vi.fn(),
    jumpToPricingGate: vi.fn(),
    ...overrides,
  }
}

beforeEach(() => { vi.clearAllMocks() })

describe('Wizard', () => {
  it('calls startFirstLaunch on first render', () => {
    const startFirstLaunch = vi.fn()
    mockUseWizardState.mockReturnValue({
      state: makeState('welcome'),
      ...makeActions({ startFirstLaunch }),
    })
    render(<Wizard onComplete={vi.fn()} />)
    expect(startFirstLaunch).toHaveBeenCalledOnce()
  })

  it('shows error alert when voice-guide modal is reached with no projectId', () => {
    mockUseWizardState.mockReturnValue({
      state: makeState('voice-guide'),
      ...makeActions(),
    })
    render(<Wizard onComplete={vi.fn()} />)
    expect(screen.getByRole('alert')).toBeInTheDocument()
  })

  it('shows error alert when connect-repo modal is reached with no projectId', () => {
    mockUseWizardState.mockReturnValue({
      state: makeState('connect-repo'),
      ...makeActions(),
    })
    render(<Wizard onComplete={vi.fn()} />)
    expect(screen.getByRole('alert')).toBeInTheDocument()
  })

  it('invokes set_wizard_completed and calls onComplete when done is dismissed', async () => {
    const onComplete = vi.fn()
    mockInvoke.mockResolvedValue(undefined)
    mockUseWizardState.mockReturnValue({
      state: makeState('done'),
      ...makeActions(),
    })
    render(<Wizard onComplete={onComplete} />)
    fireEvent.click(screen.getByRole('button', { name: /open postlane/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_wizard_completed')
      expect(onComplete).toHaveBeenCalledOnce()
    })
  })
})
