// SPDX-License-Identifier: BUSL-1.1
// Branch-coverage tests for EditPostView — split from EditPostView.test.tsx
// to keep both files within the 400-line limit.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import '@testing-library/jest-dom'
import type { DraftPost, PublishedPost, Project, ViewSelection } from '../types'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('../context/DraftPostsProvider', () => ({ useDraftPostsContext: vi.fn() }))

import { invoke } from '../ipc/invoke'
import { useDraftPostsContext } from '../context/DraftPostsProvider'
import EditPostView, { type EditPostViewProps } from './EditPostView'

const mockInvoke = vi.mocked(invoke)
const mockCtx = vi.mocked(useDraftPostsContext)
const mockRefresh = vi.fn()

function makeDraft(overrides: Partial<DraftPost> = {}): DraftPost {
  return {
    repo_id: 'r1', repo_name: 'MyRepo', repo_path: '/repo1',
    post_folder: 'post-001', platforms: ['x'], platform: 'x',
    text: 'Hello world', status: 'ready', trigger: null, error: null,
    image_url: null, project_id: 'proj-1', schedule: null,
    platform_results: null, llm_model: null, created_at: null, scheduled_for: null,
    ...overrides,
  }
}

function makePublished(overrides: Partial<PublishedPost> = {}): PublishedPost {
  return {
    repo_id: 'r1', repo_name: 'MyRepo', repo_path: '/repo1',
    post_folder: 'post-001', platforms: ['x'], platform: 'x',
    status: 'sent', scheduler_ids: {}, platform_urls: {},
    provider: null, sent_at: '2024-01-01T10:00:00Z',
    schedule: null, platform_results: null, llm_model: null, created_at: null,
    project_id: 'proj-1', text: 'Published post',
    ...overrides,
  }
}

function makeProject(overrides: Partial<Project> = {}): Project {
  return {
    id: 'proj-1', name: 'Postlane', workspace_type: 'organization',
    tier: 'free', billing_active: true, is_owner: true,
    ...overrides,
  }
}

const DEFAULT_NAV_SEL: ViewSelection = { view: 'org_queue', projectId: 'proj-1' }

function renderEdit(overrides: Partial<EditPostViewProps> = {}) {
  return render(
    <EditPostView
      post={makeDraft()} project={makeProject()} isHistory={false}
      timezone="UTC" onBack={vi.fn()} onApproved={vi.fn()}
      onNavigate={vi.fn()} pendingNavSel={null} onNavCancelled={vi.fn()}
      {...overrides}
    />,
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  mockCtx.mockReturnValue({ drafts: [], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
  mockInvoke.mockResolvedValue(null)
})

afterEach(() => { vi.useRealTimers() })

// ── Delete error ───────────────────────────────────────────────────────────────

describe('EditPostView — Delete error', () => {
  it('shows delete error message when delete_post fails', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'delete_post') throw new Error('permission denied');
      return null;
    })
    renderEdit()
    fireEvent.click(screen.getByRole('button', { name: /Delete/i }))
    fireEvent.click(screen.getByTestId('confirm-delete'))
    await waitFor(() => expect(screen.getByText(/permission denied/i)).toBeInTheDocument())
  })
})

// ── Approve error ──────────────────────────────────────────────────────────────

describe('EditPostView — Approve error', () => {
  it('shows approve error alert when approve_post fails', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'approve_post') throw new Error('server error');
      return null;
    })
    renderEdit()
    fireEvent.click(screen.getByRole('button', { name: /Approve/i }))
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
  })
})

// ── Nav guard — clean navigation ───────────────────────────────────────────────

describe('EditPostView — nav guard clean navigation', () => {
  it('calls onNavigate immediately when pendingNavSel is set and content is clean', () => {
    const onNavigate = vi.fn()
    const { rerender } = renderEdit({ pendingNavSel: null, onNavigate })
    rerender(
      <EditPostView post={makeDraft()} project={makeProject()} isHistory={false}
        timezone="UTC" onBack={vi.fn()} onApproved={vi.fn()}
        onNavigate={onNavigate} pendingNavSel={DEFAULT_NAV_SEL} onNavCancelled={vi.fn()} />,
    )
    expect(onNavigate).toHaveBeenCalledWith(DEFAULT_NAV_SEL)
  })
})

// ── Platform-specific char counting ───────────────────────────────────────────

describe('EditPostView — bluesky char counting', () => {
  it('shows Bluesky tab as selected for bluesky platform', () => {
    renderEdit({ post: makeDraft({ platforms: ['bluesky'], platform: 'bluesky', text: 'Hello' }) })
    expect(screen.getByRole('tab', { name: /bluesky/i })).toHaveAttribute('aria-selected', 'true')
  })

  it('shows correct char count for bluesky (full URL length, 300 limit)', () => {
    const draft = makeDraft({ platform: 'bluesky', text: 'a'.repeat(10) })
    renderEdit({ post: draft })
    expect(screen.getByText('10 / 300')).toBeInTheDocument()
  })
})

describe('EditPostView — mastodon char counting', () => {
  it('shows mastodon tab as selected for mastodon platform', () => {
    const draft = makeDraft({ platforms: ['mastodon'], platform: 'mastodon', text: 'Hello' })
    renderEdit({ post: draft })
    expect(screen.getByRole('tab', { name: /mastodon/i })).toHaveAttribute('aria-selected', 'true')
  })

  it('shows correct char count for mastodon (500 limit)', () => {
    const draft = makeDraft({ platform: 'mastodon', text: 'a'.repeat(10) })
    renderEdit({ post: draft })
    expect(screen.getByText('10 / 500')).toBeInTheDocument()
  })
})

describe('EditPostView — linkedin char counting', () => {
  it('shows correct char count for linkedin (3000 limit)', () => {
    const draft = makeDraft({ platform: 'linkedin', text: 'a'.repeat(50) })
    renderEdit({ post: draft })
    expect(screen.getByText('50 / 3000')).toBeInTheDocument()
  })
})

describe('EditPostView — unknown platform char counting', () => {
  it('falls back to Unicode scalar count for unknown platform (no char limit shown)', () => {
    const draft = makeDraft({ platforms: ['unknown_platform'], platform: 'unknown_platform', text: 'abc' })
    renderEdit({ post: draft })
    expect(screen.getByRole('tab', { name: /unknown_platform/i })).toBeInTheDocument()
  })
})

// ── OG image error handling ────────────────────────────────────────────────────

describe('EditPostView — OG image fetch error', () => {
  it('shows no image when fetch_og_image rejects', async () => {
    vi.useFakeTimers()
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'fetch_og_image') throw new Error('network error')
      return null
    })
    const draft = makeDraft({ text: 'Check https://example.com for details' })
    renderEdit({ post: draft })
    await act(async () => { await vi.advanceTimersByTimeAsync(500) })
    expect(screen.queryByTestId('og-image')).not.toBeInTheDocument()
  })
})

// ── onDirtyChange callback ─────────────────────────────────────────────────────

describe('EditPostView — onDirtyChange', () => {
  it('calls onDirtyChange(true) when text becomes dirty', () => {
    const onDirtyChange = vi.fn()
    renderEdit({ onDirtyChange })
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed' } })
    expect(onDirtyChange).toHaveBeenCalledWith(true)
  })

  it('calls onDirtyChange(false) on unmount', () => {
    const onDirtyChange = vi.fn()
    const { unmount } = renderEdit({ onDirtyChange })
    unmount()
    expect(onDirtyChange).toHaveBeenCalledWith(false)
  })
})

// ── DeleteModal background click ───────────────────────────────────────────────

describe('EditPostView — DeleteModal background click cancels', () => {
  it('hides delete confirmation when modal background is clicked', () => {
    renderEdit()
    fireEvent.click(screen.getByRole('button', { name: /Delete/i }))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('dialog').querySelector('.modal-background') as Element)
    expect(screen.queryByTestId('confirm-delete')).not.toBeInTheDocument()
  })
})

// ── Cmd+Enter: over limit does not approve ─────────────────────────────────────

describe('EditPostView — Cmd+Enter skips approve when over limit', () => {
  it('does not approve when clean but text is over platform limit', async () => {
    const onApproved = vi.fn()
    const draft = makeDraft({ platform: 'x', text: 'a'.repeat(281) })
    renderEdit({ post: draft, onApproved })
    fireEvent.keyDown(document, { key: 'Enter', metaKey: true })
    await waitFor(() => expect(mockInvoke).not.toHaveBeenCalledWith('approve_post', expect.anything()))
    expect(onApproved).not.toHaveBeenCalled()
  })
})

// ── Cmd+Enter: history mode does not approve ───────────────────────────────────

describe('EditPostView — Cmd+Enter skips approve in history mode', () => {
  it('does not approve when in history mode', async () => {
    const onApproved = vi.fn()
    renderEdit({ post: makePublished(), isHistory: true, onApproved })
    fireEvent.keyDown(document, { key: 'Enter', metaKey: true })
    await waitFor(() => expect(mockInvoke).not.toHaveBeenCalledWith('approve_post', expect.anything()))
    expect(onApproved).not.toHaveBeenCalled()
  })
})

// ── Ctrl+Enter (non-Mac) ───────────────────────────────────────────────────────

describe('EditPostView — Ctrl+Enter', () => {
  it('approves when clean and within limit using ctrlKey', async () => {
    const onApproved = vi.fn()
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'approve_post') return undefined; return null })
    renderEdit({ onApproved })
    fireEvent.keyDown(document, { key: 'Enter', ctrlKey: true })
    await waitFor(() => expect(onApproved).toHaveBeenCalled())
  })

  it('ignores non-Enter key with metaKey', () => {
    const onApproved = vi.fn()
    renderEdit({ onApproved })
    fireEvent.keyDown(document, { key: 'a', metaKey: true })
    expect(mockInvoke).not.toHaveBeenCalledWith('approve_post', expect.anything())
  })
})

// ── post.text null handling ────────────────────────────────────────────────────

describe('EditPostView — null post text', () => {
  it('renders empty textarea when post text is null', () => {
    const draft = makeDraft({ text: null as unknown as string })
    renderEdit({ post: draft })
    const textarea = screen.getByRole('textbox', { name: /post content/i })
    expect(textarea).toHaveValue('')
  })
})

// ── post.platform null handling ────────────────────────────────────────────────

describe('EditPostView — null post platform', () => {
  it('renders without crashing when post platform is null', () => {
    const draft = makeDraft({ platform: null as unknown as string })
    renderEdit({ post: draft })
    expect(screen.getByRole('textbox', { name: /post content/i })).toBeInTheDocument()
  })

  it('calls save_post_draft with empty string platform when platform is null', async () => {
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'save_post_draft') return undefined; return null })
    const draft = makeDraft({ platform: null as unknown as string, text: 'original' })
    renderEdit({ post: draft })
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'updated' } })
    fireEvent.click(screen.getByRole('button', { name: /Save/i }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('save_post_draft', expect.objectContaining({ platform: '' })))
  })

  it('calls approve_post with empty string platform when platform is null', async () => {
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'approve_post') return undefined; return null })
    const draft = makeDraft({ platform: null as unknown as string })
    renderEdit({ post: draft })
    fireEvent.click(screen.getByRole('button', { name: /Approve/i }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('approve_post', expect.objectContaining({ platform: '' })))
  })
})

// ── DeleteModal with unknown platform ─────────────────────────────────────────

describe('EditPostView — DeleteModal unknown platform label', () => {
  it('shows raw platform string in delete dialog for unlabelled platform', () => {
    const draft = makeDraft({ platform: 'mastodon' })
    renderEdit({ post: draft })
    fireEvent.click(screen.getByRole('button', { name: /Delete/i }))
    const dialog = screen.getByRole('dialog')
    expect(dialog.textContent).toMatch(/mastodon/i)
  })
})
