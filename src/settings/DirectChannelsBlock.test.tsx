// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))

import { invoke } from '../ipc/invoke'
import { openUrl } from '@tauri-apps/plugin-opener'
import DirectChannelsBlock from './DirectChannelsBlock'

const mockInvoke = vi.mocked(invoke)
const mockOpen = vi.mocked(openUrl)

beforeEach(() => {
  vi.clearAllMocks()
  // default: not connected
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'get_mastodon_connected_account') return null
    return null
  })
})

// ── Section structure ─────────────────────────────────────────────────────────

describe('DirectChannelsBlock — section structure', () => {
  it('renders "Direct social channels" heading', async () => {
    render(<DirectChannelsBlock />)
    expect(await screen.findByText('Direct social channels')).toBeInTheDocument()
  })

  it('renders Mastodon row', async () => {
    render(<DirectChannelsBlock />)
    expect(await screen.findByText('Mastodon')).toBeInTheDocument()
  })

  it('shows Connect button when not connected', async () => {
    render(<DirectChannelsBlock />)
    expect(await screen.findByRole('button', { name: /connect/i })).toBeInTheDocument()
  })

  it('does not show the instance form on initial render', async () => {
    render(<DirectChannelsBlock />)
    await screen.findByRole('button', { name: /connect/i })
    expect(screen.queryByPlaceholderText(/mastodon/i)).not.toBeInTheDocument()
  })
})

// ── Already connected on mount ────────────────────────────────────────────────

describe('DirectChannelsBlock — connected on mount', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_mastodon_connected_account')
        return { instance: 'mastodon.social', username: 'postlane' }
      return null
    })
  })

  it('shows the connected instance name', async () => {
    render(<DirectChannelsBlock />)
    expect(await screen.findByText('mastodon.social')).toBeInTheDocument()
  })

  it('shows the connected username', async () => {
    render(<DirectChannelsBlock />)
    expect(await screen.findByText('@postlane')).toBeInTheDocument()
  })

  it('shows Disconnect button when connected', async () => {
    render(<DirectChannelsBlock />)
    expect(await screen.findByRole('button', { name: /disconnect/i })).toBeInTheDocument()
  })

  it('does not show Connect button when already connected', async () => {
    render(<DirectChannelsBlock />)
    await screen.findByRole('button', { name: /disconnect/i })
    expect(screen.queryByRole('button', { name: /^connect$/i })).not.toBeInTheDocument()
  })
})

// ── Expand / collapse ─────────────────────────────────────────────────────────

describe('DirectChannelsBlock — expand and cancel', () => {
  it('clicking Connect expands the instance form', async () => {
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    expect(await screen.findByPlaceholderText(/mastodon instance/i)).toBeInTheDocument()
  })

  it('clicking Cancel collapses the form', async () => {
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    await screen.findByPlaceholderText(/mastodon instance/i)
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    expect(screen.queryByPlaceholderText(/mastodon instance/i)).not.toBeInTheDocument()
  })

  it('Connect button hidden while form is expanded', async () => {
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    await screen.findByPlaceholderText(/mastodon instance/i)
    expect(screen.queryByRole('button', { name: /^connect$/i })).not.toBeInTheDocument()
  })
})

// ── Instance validation ───────────────────────────────────────────────────────

describe('DirectChannelsBlock — instance validation', () => {
  it('shows error when instance contains "://"', async () => {
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    fireEvent.change(await screen.findByPlaceholderText(/mastodon instance/i), {
      target: { value: 'https://mastodon.social' },
    })
    fireEvent.click(screen.getByRole('button', { name: /^submit$/i }))
    expect(screen.getByText(/hostname only/i)).toBeInTheDocument()
  })

  it('Submit button enabled when input has text', async () => {
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    fireEvent.change(await screen.findByPlaceholderText(/mastodon instance/i), {
      target: { value: 'mastodon.social' },
    })
    expect(screen.getByRole('button', { name: /^submit$/i })).toBeEnabled()
  })

  it('shows "Instance not found" when instance check fails', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_mastodon_connected_account') return null
      if (cmd === 'get_mastodon_char_limit') throw new Error('unreachable')
      return null
    })
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    fireEvent.change(await screen.findByPlaceholderText(/mastodon instance/i), {
      target: { value: 'bad.instance' },
    })
    fireEvent.click(screen.getByRole('button', { name: /^submit$/i }))
    await waitFor(() => expect(screen.getByText(/instance not found/i)).toBeInTheDocument())
  })
})

// ── OAuth flow ────────────────────────────────────────────────────────────────

async function openAndConnect(instance = 'mastodon.social') {
  render(<DirectChannelsBlock />)
  fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
  fireEvent.change(await screen.findByPlaceholderText(/mastodon instance/i), {
    target: { value: instance },
  })
  fireEvent.click(screen.getByRole('button', { name: /^submit$/i }))
}

async function saveConnectionFlow() {
  render(<DirectChannelsBlock />)
  fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
  fireEvent.change(await screen.findByPlaceholderText(/mastodon instance/i), { target: { value: 'mastodon.social' } })
  fireEvent.click(screen.getByRole('button', { name: /^submit$/i }))
  await screen.findByPlaceholderText(/one time code/i)
  fireEvent.change(screen.getByPlaceholderText(/one time code/i), { target: { value: 'abc123' } })
  fireEvent.click(screen.getByRole('button', { name: /^submit$/i }))
}

describe('DirectChannelsBlock — OAuth flow', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_mastodon_connected_account') return null
      if (cmd === 'get_mastodon_char_limit') return 500
      if (cmd === 'register_mastodon_app') return 'https://mastodon.social/oauth/authorize?client_id=abc'
      if (cmd === 'exchange_mastodon_code') return 'alice'
      return null
    })
  })

  it('calls register_mastodon_app on Submit', async () => {
    await openAndConnect()
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('register_mastodon_app', { instance: 'mastodon.social' })
    )
  })

  it('opens auth URL in browser after Submit', async () => {
    await openAndConnect()
    await waitFor(() =>
      expect(mockOpen).toHaveBeenCalledWith('https://mastodon.social/oauth/authorize?client_id=abc')
    )
  })

  it('shows code entry placeholder after Submit succeeds', async () => {
    await openAndConnect()
    expect(await screen.findByPlaceholderText(/one time code/i)).toBeInTheDocument()
  })

  it('calls exchange_mastodon_code with instance and code on Save', async () => {
    await saveConnectionFlow()
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('exchange_mastodon_code', { instance: 'mastodon.social', code: 'abc123' })
    )
  })

  it('shows connected username and instance after Save', async () => {
    await saveConnectionFlow()
    expect(await screen.findByText('@alice')).toBeInTheDocument()
    expect(screen.getByText('mastodon.social')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /disconnect/i })).toBeInTheDocument()
  })
})

// ── Disconnect ────────────────────────────────────────────────────────────────

describe('DirectChannelsBlock — Disconnect', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_mastodon_connected_account')
        return { instance: 'mastodon.social', username: 'postlane' }
      if (cmd === 'disconnect_mastodon') return null
      return null
    })
  })

  it('shows confirmation dialog before disconnecting', async () => {
    const spy = vi.spyOn(window, 'confirm').mockReturnValueOnce(false)
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /disconnect/i }))
    expect(spy).toHaveBeenCalled()
  })

  it('calls disconnect_mastodon when confirmed', async () => {
    vi.spyOn(window, 'confirm').mockReturnValueOnce(true)
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /disconnect/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('disconnect_mastodon', { instance: 'mastodon.social' })
    )
  })

  it('resets to Connect button after disconnect', async () => {
    vi.spyOn(window, 'confirm').mockReturnValueOnce(true)
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /disconnect/i }))
    expect(await screen.findByRole('button', { name: /^connect$/i })).toBeInTheDocument()
  })

  it('does not call disconnect_mastodon when cancelled', async () => {
    vi.spyOn(window, 'confirm').mockReturnValueOnce(false)
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /disconnect/i }))
    expect(mockInvoke).not.toHaveBeenCalledWith('disconnect_mastodon', expect.anything())
  })
})
