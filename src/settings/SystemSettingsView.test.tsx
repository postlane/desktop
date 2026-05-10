// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { invoke } from '../ipc/invoke'
import SystemSettingsView from './SystemSettingsView'

const mockInvoke = vi.mocked(invoke)

function makeWatcherStatus(overrides = {}) {
  return { repo_name: 'MyRepo', repo_path: '/repos/myrepo', active: false, last_event_at: null, ...overrides }
}

function makeModelStats(overrides = {}) {
  return { edit_rate: 0, edited_posts: 0, total_posts: 0, denominator_unit: 'platform_approval', pre_m19_post_count: 0, ...overrides }
}

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'get_app_version') return '1.2.3'
    if (cmd === 'get_watcher_status') return []
    if (cmd === 'get_model_stats') return makeModelStats()
    return null
  })
})

// ── Load ────────────────────────────────────────────────────────────────────────

describe('SystemSettingsView — load', () => {
  it('calls get_app_version on mount', async () => {
    render(<SystemSettingsView />)
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_app_version'))
  })

  it('calls get_watcher_status on mount', async () => {
    render(<SystemSettingsView />)
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_watcher_status'))
  })

  it('displays the app version', async () => {
    render(<SystemSettingsView />)
    await waitFor(() => expect(screen.getByText('1.2.3')).toBeInTheDocument())
  })
})

// ── Watcher health ──────────────────────────────────────────────────────────────

describe('SystemSettingsView — watcher health', () => {
  it('shows a table of watcher statuses', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_app_version') return '1.0.0'
      if (cmd === 'get_watcher_status') return [
        makeWatcherStatus({ repo_name: 'BlogRepo', active: true }),
        makeWatcherStatus({ repo_name: 'DocsRepo', active: false }),
      ]
      return null
    })
    render(<SystemSettingsView />)
    await waitFor(() => {
      expect(screen.getByText('BlogRepo')).toBeInTheDocument()
      expect(screen.getByText('DocsRepo')).toBeInTheDocument()
    })
  })

  it('shows active status for a running watcher', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_app_version') return '1.0.0'
      if (cmd === 'get_watcher_status') return [makeWatcherStatus({ repo_name: 'ActiveRepo', active: true })]
      return null
    })
    render(<SystemSettingsView />)
    await waitFor(() => expect(screen.getByText('Active')).toBeInTheDocument())
  })

  it('shows inactive status for a stopped watcher', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_app_version') return '1.0.0'
      if (cmd === 'get_watcher_status') return [makeWatcherStatus({ repo_name: 'InactiveRepo', active: false })]
      return null
    })
    render(<SystemSettingsView />)
    await waitFor(() => expect(screen.getByText('Inactive')).toBeInTheDocument())
  })

  it('shows empty message when no repos configured', async () => {
    render(<SystemSettingsView />)
    await waitFor(() => expect(screen.getByText(/no repositories/i)).toBeInTheDocument())
  })
})

// ── Check for updates ───────────────────────────────────────────────────────────

describe('SystemSettingsView — check for updates', () => {
  it('renders a check for updates button', async () => {
    render(<SystemSettingsView />)
    await waitFor(() => expect(screen.getByRole('button', { name: /check for updates/i })).toBeInTheDocument())
  })

  it('check for updates button is disabled', async () => {
    render(<SystemSettingsView />)
    await waitFor(() => expect(screen.getByRole('button', { name: /check for updates/i })).toBeDisabled())
  })
})

// ── AI draft quality (ModelStats) ───────────────────────────────────────────────

describe('SystemSettingsView — AI draft quality', () => {
  it('calls get_model_stats on mount', async () => {
    render(<SystemSettingsView />)
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_model_stats'))
  })

  it('shows "No data yet" when total_posts is 0', async () => {
    render(<SystemSettingsView />)
    await waitFor(() => expect(screen.getByText(/no data yet/i)).toBeInTheDocument())
  })

  it('renders edit_rate as a percentage with 1 decimal place', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_app_version') return '1.0.0'
      if (cmd === 'get_watcher_status') return []
      if (cmd === 'get_model_stats') return makeModelStats({ edit_rate: 0.333, edited_posts: 1, total_posts: 3 })
      return null
    })
    render(<SystemSettingsView />)
    await waitFor(() => expect(screen.getByText('33.3%')).toBeInTheDocument())
  })

  it('renders edited_posts and total_posts counts', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_app_version') return '1.0.0'
      if (cmd === 'get_watcher_status') return []
      if (cmd === 'get_model_stats') return makeModelStats({ edit_rate: 0.5, edited_posts: 5, total_posts: 10 })
      return null
    })
    render(<SystemSettingsView />)
    await waitFor(() => {
      expect(screen.getByText(/5.*edited/i)).toBeInTheDocument()
      expect(screen.getByText(/10.*post/i)).toBeInTheDocument()
    })
  })

  it('shows error message when get_model_stats fails', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_app_version') return '1.0.0'
      if (cmd === 'get_watcher_status') return []
      if (cmd === 'get_model_stats') throw new Error('stats unavailable')
      return null
    })
    render(<SystemSettingsView />)
    await waitFor(() => expect(screen.getByText(/could not load/i)).toBeInTheDocument())
  })
})
