// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useDirtyNavGuard } from './useAppHooks'
import type { ViewSelection } from '../types'

const QUEUE_A: ViewSelection = { view: 'org_queue', projectId: 'proj-a' }
const QUEUE_B: ViewSelection = { view: 'org_queue', projectId: 'proj-b' }

function setup(currentView: ViewSelection = QUEUE_A) {
  const setCurrentView = vi.fn()
  const onSwitchAccount = vi.fn()
  const { result } = renderHook(() => useDirtyNavGuard(setCurrentView, currentView, onSwitchAccount))
  return { result, setCurrentView, onSwitchAccount }
}

describe('useDirtyNavGuard — existing nav behaviour (unchanged)', () => {
  it('navigates immediately when not dirty', () => {
    const { result, setCurrentView } = setup()
    act(() => { result.current.handleNavClick(QUEUE_B) })
    expect(setCurrentView).toHaveBeenCalledWith(QUEUE_B)
    expect(result.current.discardModalOpen).toBe(false)
  })

  it('opens the discard modal instead of navigating when dirty', () => {
    const { result, setCurrentView } = setup()
    act(() => { result.current.editPostViewDirtyRef.current = true })
    act(() => { result.current.handleNavClick(QUEUE_B) })
    expect(setCurrentView).not.toHaveBeenCalled()
    expect(result.current.discardModalOpen).toBe(true)
  })

  it('confirmDiscard navigates to the pending nav selection and clears dirty state', () => {
    const { result, setCurrentView } = setup()
    act(() => { result.current.editPostViewDirtyRef.current = true })
    act(() => { result.current.handleNavClick(QUEUE_B) })
    act(() => { result.current.confirmDiscard() })
    expect(setCurrentView).toHaveBeenCalledWith(QUEUE_B)
    expect(result.current.discardModalOpen).toBe(false)
    expect(result.current.editPostViewDirtyRef.current).toBe(false)
  })

  it('cancelDiscard closes the modal without navigating', () => {
    const { result, setCurrentView } = setup()
    act(() => { result.current.editPostViewDirtyRef.current = true })
    act(() => { result.current.handleNavClick(QUEUE_B) })
    act(() => { result.current.cancelDiscard() })
    expect(setCurrentView).not.toHaveBeenCalled()
    expect(result.current.discardModalOpen).toBe(false)
  })
})

describe('useDirtyNavGuard — account switching (checklist 24.4.10)', () => {
  it('switches immediately when not dirty', () => {
    const { result, onSwitchAccount } = setup()
    act(() => { result.current.handleAccountSwitch('account-2') })
    expect(onSwitchAccount).toHaveBeenCalledWith('account-2')
    expect(result.current.discardModalOpen).toBe(false)
  })

  it('opens the discard modal instead of switching when dirty', () => {
    const { result, onSwitchAccount } = setup()
    act(() => { result.current.editPostViewDirtyRef.current = true })
    act(() => { result.current.handleAccountSwitch('account-2') })
    expect(onSwitchAccount).not.toHaveBeenCalled()
    expect(result.current.discardModalOpen).toBe(true)
  })

  it('confirmDiscard switches to the pending account and clears dirty state', () => {
    const { result, onSwitchAccount, setCurrentView } = setup()
    act(() => { result.current.editPostViewDirtyRef.current = true })
    act(() => { result.current.handleAccountSwitch('account-2') })
    act(() => { result.current.confirmDiscard() })
    expect(onSwitchAccount).toHaveBeenCalledWith('account-2')
    expect(setCurrentView).not.toHaveBeenCalled()
    expect(result.current.discardModalOpen).toBe(false)
    expect(result.current.editPostViewDirtyRef.current).toBe(false)
  })

  it('cancelDiscard closes the modal without switching accounts', () => {
    const { result, onSwitchAccount } = setup()
    act(() => { result.current.editPostViewDirtyRef.current = true })
    act(() => { result.current.handleAccountSwitch('account-2') })
    act(() => { result.current.cancelDiscard() })
    expect(onSwitchAccount).not.toHaveBeenCalled()
    expect(result.current.discardModalOpen).toBe(false)
  })

  it('a pending nav action is not confused with a pending account-switch action', () => {
    const { result, onSwitchAccount, setCurrentView } = setup()
    act(() => { result.current.editPostViewDirtyRef.current = true })
    act(() => { result.current.handleNavClick(QUEUE_B) })
    act(() => { result.current.confirmDiscard() })
    expect(setCurrentView).toHaveBeenCalledWith(QUEUE_B)
    expect(onSwitchAccount).not.toHaveBeenCalled()
  })
})
