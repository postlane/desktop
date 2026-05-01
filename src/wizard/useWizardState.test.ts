// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useWizardState } from './useWizardState'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
import { invoke } from '@tauri-apps/api/core'
const mockInvoke = vi.mocked(invoke)

beforeEach(() => { vi.clearAllMocks() })

describe('useWizardState — navigation', () => {
  it('first launch starts at welcome', () => {
    const { result } = renderHook(() => useWizardState())
    act(() => { result.current.startFirstLaunch() })
    expect(result.current.state.modal).toBe('welcome')
  })

  it('next from sign-in is blocked without a license token', () => {
    const { result } = renderHook(() => useWizardState())
    act(() => {
      result.current.startFirstLaunch()
      result.current.next() // welcome → sign-in
      result.current.next() // blocked — no token
    })
    expect(result.current.state.modal).toBe('sign-in')
  })

  it('next from sign-in advances to name-workspace once token is present', () => {
    const { result } = renderHook(() => useWizardState())
    act(() => {
      result.current.startFirstLaunch()
      result.current.next()                       // → sign-in
      result.current.setLicenseTokenPresent(true) // token acquired
      result.current.next()                       // → name-workspace
    })
    expect(result.current.state.modal).toBe('name-workspace')
  })

  it('skip on backup-scheduler advances to map-profiles', () => {
    const { result } = renderHook(() => useWizardState())
    act(() => {
      result.current.startFirstLaunch()
      result.current.next()
      result.current.setLicenseTokenPresent(true)
      result.current.next()
      result.current.setProjectId('proj-123')
      result.current.next()
      result.current.setSchedulerConnected(true)
      result.current.next() // → backup-scheduler
      result.current.skip() // → map-profiles
    })
    expect(result.current.state.modal).toBe('map-profiles')
  })

  it('back from name-workspace goes to sign-in', () => {
    const { result } = renderHook(() => useWizardState())
    act(() => {
      result.current.startFirstLaunch()
      result.current.next()
      result.current.setLicenseTokenPresent(true)
      result.current.next() // → name-workspace
      result.current.back() // → sign-in
    })
    expect(result.current.state.modal).toBe('sign-in')
  })
})

describe('useWizardState — entry points', () => {
  it('startAddRepo starts at connect-repo with project id stored', () => {
    const { result } = renderHook(() => useWizardState())
    act(() => { result.current.startAddRepo('proj-abc') })
    expect(result.current.state.modal).toBe('connect-repo')
    expect(result.current.state.projectId).toBe('proj-abc')
  })

  it('startAddProject goes to pricing-gate when no billing slot', async () => {
    mockInvoke.mockResolvedValue('none')
    const { result } = renderHook(() => useWizardState())
    await act(async () => { await result.current.startAddProject() })
    expect(result.current.state.modal).toBe('pricing-gate')
  })
})

describe('useWizardState — jump actions', () => {
  it('jumpToDone sets modal to done from any state', () => {
    const { result } = renderHook(() => useWizardState())
    act(() => { result.current.startAddRepo('proj-1') })
    act(() => { result.current.jumpToDone() })
    expect(result.current.state.modal).toBe('done')
  })

  it('jumpToPricingGate sets modal to pricing-gate from any state', () => {
    const { result } = renderHook(() => useWizardState())
    act(() => { result.current.startAddRepo('proj-1') })
    act(() => { result.current.jumpToPricingGate() })
    expect(result.current.state.modal).toBe('pricing-gate')
  })
})

describe('useWizardState — shortcut actions', () => {
  it('advanceScheduler sets modal to backup-scheduler bypassing gate', () => {
    const { result } = renderHook(() => useWizardState())
    act(() => {
      result.current.startFirstLaunch()
      result.current.next()
      result.current.setLicenseTokenPresent(true)
      result.current.next()
      result.current.setProjectId('proj-123')
      result.current.next() // → connect-scheduler
      result.current.advanceScheduler()
    })
    expect(result.current.state.modal).toBe('backup-scheduler')
  })

  it('advancePastGate sets modal to name-workspace', () => {
    const { result } = renderHook(() => useWizardState())
    act(() => { result.current.advancePastGate() })
    expect(result.current.state.modal).toBe('name-workspace')
  })
})
