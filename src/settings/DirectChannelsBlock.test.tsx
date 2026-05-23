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
    if (cmd === 'get_mastodon_connected_instance') return null
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
    expect(screen.queryByPlaceholderText(/mastodon\.social/i)).not.toBeInTheDocument()
  })
})

// ── Already connected on mount ────────────────────────────────────────────────

describe('DirectChannelsBlock — connected on mount', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_mastodon_connected_instance') return 'mastodon.social'
      return null
    })
  })

  it('shows the connected instance name', async () => {
    render(<DirectChannelsBlock />)
    expect(await screen.findByText('mastodon.social')).toBeInTheDocument()
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
    expect(await screen.findByPlaceholderText(/mastodon\.social/i)).toBeInTheDocument()
  })

  it('clicking Cancel collapses the form', async () => {
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    await screen.findByPlaceholderText(/mastodon\.social/i)
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    expect(screen.queryByPlaceholderText(/mastodon\.social/i)).not.toBeInTheDocument()
  })

  it('Connect button hidden while form is expanded', async () => {
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    await screen.findByPlaceholderText(/mastodon\.social/i)
    expect(screen.queryByRole('button', { name: /^connect$/i })).not.toBeInTheDocument()
  })
})

// ── Instance validation (input) ───────────────────────────────────────────────

describe('DirectChannelsBlock — instance validation', () => {
  it('shows error when instance contains "://"', async () => {
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    fireEvent.change(await screen.findByPlaceholderText(/mastodon\.social/i), {
      target: { value: 'https://mastodon.social' },
    })
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }))
    expect(screen.getByText(/hostname only/i)).toBeInTheDocument()
  })

  it('Connect button disabled until instance validated', async () => {
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    await screen.findByPlaceholderText(/mastodon\.social/i)
    expect(screen.getByRole('button', { name: /^connect to mastodon$/i })).toBeDisabled()
  })
})

// ── Instance validation (API calls) ──────────────────────────────────────────

describe('DirectChannelsBlock — instance validation (API calls)', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_mastodon_connected_instance') return null
      if (cmd === 'get_mastodon_char_limit') return 500
      return null
    })
  })

  it('calls get_mastodon_char_limit on Test instance', async () => {
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    fireEvent.change(await screen.findByPlaceholderText(/mastodon\.social/i), {
      target: { value: 'mastodon.social' },
    })
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('get_mastodon_char_limit', { instance: 'mastodon.social' })
    )
  })

  it('enables Connect to Mastodon after successful test', async () => {
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    fireEvent.change(await screen.findByPlaceholderText(/mastodon\.social/i), {
      target: { value: 'mastodon.social' },
    })
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }))
    await waitFor(() => expect(screen.getByRole('button', { name: /^connect to mastodon$/i })).toBeEnabled())
  })

  it('shows "Instance not found" when test fails', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_mastodon_connected_instance') return null
      if (cmd === 'get_mastodon_char_limit') throw new Error('unreachable')
      return null
    })
    render(<DirectChannelsBlock />)
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
    fireEvent.change(await screen.findByPlaceholderText(/mastodon\.social/i), {
      target: { value: 'bad.instance' },
    })
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }))
    await waitFor(() => expect(screen.getByText(/instance not found/i)).toBeInTheDocument())
  })
})

// ── OAuth flow ────────────────────────────────────────────────────────────────

async function openAndValidate(instance = 'mastodon.social') {
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'get_mastodon_connected_instance') return null
    if (cmd === 'get_mastodon_char_limit') return 500
    if (cmd === 'register_mastodon_app') return `https://${instance}/oauth/authorize?client_id=abc`
    return null
  })
  render(<DirectChannelsBlock />)
  fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
  fireEvent.change(await screen.findByPlaceholderText(/mastodon\.social/i), {
    target: { value: instance },
  })
  fireEvent.click(screen.getByRole('button', { name: /test instance/i }))
  await waitFor(() => expect(screen.getByRole('button', { name: /^connect to mastodon$/i })).toBeEnabled())
}

async function saveConnectionFlow() {
  render(<DirectChannelsBlock />)
  fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }))
  fireEvent.change(await screen.findByPlaceholderText(/mastodon\.social/i), { target: { value: 'mastodon.social' } })
  fireEvent.click(screen.getByRole('button', { name: /test instance/i }))
  await waitFor(() => screen.getByRole('button', { name: /^connect to mastodon$/i }))
  fireEvent.click(screen.getByRole('button', { name: /^connect to mastodon$/i }))
  await screen.findByPlaceholderText(/paste the code/i)
  fireEvent.change(screen.getByPlaceholderText(/paste the code/i), { target: { value: 'abc123' } })
  fireEvent.click(screen.getByRole('button', { name: /save/i }))
}

describe('DirectChannelsBlock — OAuth flow', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_mastodon_connected_instance') return null
      if (cmd === 'get_mastodon_char_limit') return 500
      if (cmd === 'register_mastodon_app') return 'https://mastodon.social/oauth/authorize?client_id=abc'
      if (cmd === 'exchange_mastodon_code') return 'alice'
      return null
    })
  })

  it('calls register_mastodon_app on Connect to Mastodon', async () => {
    await openAndValidate()
    fireEvent.click(screen.getByRole('button', { name: /^connect to mastodon$/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('register_mastodon_app', { instance: 'mastodon.social' })
    )
  })

  it('opens auth URL in browser after Connect to Mastodon', async () => {
    await openAndValidate()
    fireEvent.click(screen.getByRole('button', { name: /^connect to mastodon$/i }))
    await waitFor(() =>
      expect(mockOpen).toHaveBeenCalledWith('https://mastodon.social/oauth/authorize?client_id=abc')
    )
  })

  it('shows code entry form after Connect to Mastodon succeeds', async () => {
    await openAndValidate()
    fireEvent.click(screen.getByRole('button', { name: /^connect to mastodon$/i }))
    expect(await screen.findByPlaceholderText(/paste the code/i)).toBeInTheDocument()
  })

  it('calls exchange_mastodon_code with instance and code on Save', async () => {
    await saveConnectionFlow()
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('exchange_mastodon_code', { instance: 'mastodon.social', code: 'abc123' })
    )
  })

  it('shows connected instance after Save', async () => {
    await saveConnectionFlow()
    expect(await screen.findByText('mastodon.social')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /disconnect/i })).toBeInTheDocument()
  })
})

// ── Disconnect ────────────────────────────────────────────────────────────────

describe('DirectChannelsBlock — Disconnect', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_mastodon_connected_instance') return 'mastodon.social'
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
