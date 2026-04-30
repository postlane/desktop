// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { loadSchedulerCreds } from './SchedulerTab'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
const mockInvoke = vi.mocked(invoke)

beforeEach(() => vi.clearAllMocks())

describe('loadSchedulerCreds — cancellation', () => {
  it('does not call onCred when cancelled before invoke resolves', async () => {
    const resolvers: Array<(v: string) => void> = []
    mockInvoke.mockImplementation(
      () => new Promise<string>(res => { resolvers.push(res) })
    )

    let cancelled = false
    const onCred = vi.fn()

    const p = loadSchedulerCreds(() => cancelled, onCred)
    cancelled = true
    resolvers.forEach(r => r('key-preview'))
    await p

    expect(onCred).not.toHaveBeenCalled()
  })

  it('calls onCred when not cancelled', async () => {
    mockInvoke.mockResolvedValue('key-preview')
    const onCred = vi.fn()

    await loadSchedulerCreds(() => false, onCred)

    expect(onCred).toHaveBeenCalled()
  })
})
