// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }))

import { invoke } from '../ipc/invoke'
import { openUrl } from '@tauri-apps/plugin-opener'
import { startLinkProviderAccountFlow } from './providerAccountLinking'

const mockInvoke = vi.mocked(invoke)
const mockOpenUrl = vi.mocked(openUrl)

beforeEach(() => {
  vi.clearAllMocks()
})

describe('startLinkProviderAccountFlow', () => {
  it('opens the login URL with the local server port and link_provider_account mode', async () => {
    mockInvoke.mockResolvedValueOnce(47312)

    startLinkProviderAccountFlow()

    await vi.waitFor(() => expect(mockOpenUrl).toHaveBeenCalledWith(
      'https://postlane.dev/login?desktop=1&port=47312&mode=link_provider_account',
    ))
  })

  it('opens the login URL without a port when get_local_server_port fails', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('server not ready'))
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})

    startLinkProviderAccountFlow()

    await vi.waitFor(() => expect(mockOpenUrl).toHaveBeenCalledWith(
      'https://postlane.dev/login?desktop=1&mode=link_provider_account',
    ))
    consoleError.mockRestore()
  })
})
