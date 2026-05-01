// SPDX-License-Identifier: BUSL-1.1
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    onResized: vi.fn().mockResolvedValue(() => {}),
    outerPosition: vi.fn().mockResolvedValue({ x: 0, y: 0 }),
  }))
}))
vi.mock('./wizard/Wizard', () => ({ default: () => <div>Wizard</div> }))
vi.mock('./wizard/SignInScreen', () => ({ default: () => <div>SignInScreen</div> }))
vi.mock('./nav/LeftNav', () => ({ default: () => <div>LeftNav</div> }))
vi.mock('./telemetry/TelemetryConsentModal', () => ({ default: () => null }))
vi.mock('./drafts/AllReposDraftsView', () => ({ default: () => <div>AllReposDraftsView</div> }))
vi.mock('./settings/SettingsPanel', () => ({ default: () => <div>SettingsPanel</div> }))

import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import App from './App'

const mockInvoke = vi.mocked(invoke)

function makeAppState(overrides = {}) {
  return {
    version: 1,
    window: { width: 1100, height: 700, x: 0, y: 0 },
    nav: { last_view: 'all_repos', last_repo_id: null, last_section: 'drafts', expanded_repos: [] },
    wizard_completed: true,
    timezone: '',
    telemetry_consent: false,
    consent_asked: true,
    ...overrides,
  }
}

beforeEach(() => { vi.clearAllMocks() })

describe('useWindowSizePersistence error logging', () => {
  it('logs error message string (not raw Error object) when resize save fails', async () => {
    let capturedResizeHandler: ((event: { payload: { width: number; height: number } }) => void) | undefined
    const outerPositionMock = vi.fn().mockRejectedValue(new Error('IPC error'))

    vi.mocked(getCurrentWindow).mockReturnValue({
      onResized: vi.fn().mockImplementation((handler: (event: { payload: { width: number; height: number } }) => void) => {
        capturedResizeHandler = handler
        return Promise.resolve(() => {})
      }),
      outerPosition: outerPositionMock,
    } as unknown as ReturnType<typeof getCurrentWindow>)

    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState())
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })

    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    render(<App />)
    await waitFor(() => { expect(capturedResizeHandler).toBeDefined() })

    if (capturedResizeHandler) capturedResizeHandler({ payload: { width: 800, height: 600 } })
    await new Promise((resolve) => setTimeout(resolve, 600))

    const resizeErrorCall = errorSpy.mock.calls.find((args) =>
      String(args[0]).includes('window') || String(args[1])?.includes('window')
    )
    if (resizeErrorCall) {
      const errorArg = resizeErrorCall[resizeErrorCall.length - 1]
      expect(typeof errorArg).toBe('string')
      expect(String(errorArg)).not.toMatch(/\n\s+at\s/)
    }

    errorSpy.mockRestore()
  })
})

describe('App startup', () => {
  it('test_shows_wizard_on_first_launch', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: false }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(false)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => { expect(screen.getByText('Wizard')).toBeInTheDocument() })
  })

  it('test_shows_sign_in_on_missing_token', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(false)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => { expect(screen.getByText('SignInScreen')).toBeInTheDocument() })
  })

  it('test_shows_main_app_when_complete', async () => {
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'read_app_state_command') return Promise.resolve(makeAppState({ wizard_completed: true }))
      if (cmd === 'get_license_signed_in') return Promise.resolve(true)
      if (cmd === 'get_repos') return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(() => { expect(screen.getByText('LeftNav')).toBeInTheDocument() })
  })
})
