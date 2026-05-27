// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import type { DraftPost, Project, ViewSelection } from '../types'

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

function makeProject(overrides: Partial<Project> = {}): Project {
  return {
    id: 'proj-1', name: 'Postlane', workspace_type: 'organization',
    tier: 'free', billing_active: true, is_owner: true,
    ...overrides,
  }
}

const DEFAULT_VIEW: ViewSelection = { view: 'org_queue', projectId: 'proj-1' }

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

// ── Two-column layout ─────────────────────────────────────────────────────────

describe('EditPostView — two-column layout', () => {
  it('renders a tab for each platform in post.platforms', () => {
    renderEdit({ post: makeDraft({ platforms: ['x', 'bluesky'], platform: 'x' }) })
    expect(screen.getByRole('tab', { name: /x/i })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /bluesky/i })).toBeInTheDocument()
  })

  it('marks the current platform tab as selected', () => {
    renderEdit({ post: makeDraft({ platforms: ['x', 'bluesky'], platform: 'bluesky' }) })
    expect(screen.getByRole('tab', { name: /bluesky/i })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('tab', { name: /x/i })).toHaveAttribute('aria-selected', 'false')
  })

  it('Unsplash search is not inside the preview column', () => {
    renderEdit()
    const preview = screen.getByTestId('preview-column')
    const picker = screen.queryByRole('searchbox', { name: /search unsplash/i })
    if (picker) expect(preview).not.toContainElement(picker)
  })

  it('URL image input is not inside the preview column', () => {
    renderEdit()
    const preview = screen.getByTestId('preview-column')
    const urlInput = screen.queryByRole('textbox', { name: /add an image from a url/i })
    if (urlInput) expect(preview).not.toContainElement(urlInput)
  })

  it('preview column shows the current post text', () => {
    renderEdit({ post: makeDraft({ text: 'Preview this text' }) })
    expect(screen.getByTestId('preview-text')).toHaveTextContent('Preview this text')
  })

  it('Approve button is inside the preview column', () => {
    renderEdit()
    const preview = screen.getByTestId('preview-column')
    expect(preview).toContainElement(screen.getByRole('button', { name: /approve/i }))
  })
})

// ── Platform tab switching ─────────────────────────────────────────────────────

describe('EditPostView — platform tab switching', () => {
  const xPost = makeDraft({ platforms: ['x', 'bluesky'], platform: 'x', text: 'X text' })
  const bskyPost = makeDraft({ platforms: ['x', 'bluesky'], platform: 'bluesky', text: 'Bluesky text' })

  beforeEach(() => {
    mockCtx.mockReturnValue({ drafts: [xPost, bskyPost], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
  })

  it('auto-saves dirty draft before switching tabs', async () => {
    renderEdit({ post: xPost })
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'Edited X' } })
    fireEvent.click(screen.getByRole('tab', { name: /bluesky/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_post_draft',
        expect.objectContaining({ platform: 'x', text: 'Edited X' }))
    )
  })

  it('does not call save_post_draft when switching tabs with clean draft', async () => {
    renderEdit({ post: xPost })
    fireEvent.click(screen.getByRole('tab', { name: /bluesky/i }))
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: /post content/i })).toHaveValue('Bluesky text')
    )
    expect(mockInvoke).not.toHaveBeenCalledWith('save_post_draft', expect.anything())
  })

  it('loads sibling platform text after switching tabs', async () => {
    renderEdit({ post: xPost })
    fireEvent.click(screen.getByRole('tab', { name: /bluesky/i }))
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: /post content/i })).toHaveValue('Bluesky text')
    )
  })

  it('updates the selected tab after switching', async () => {
    renderEdit({ post: xPost })
    fireEvent.click(screen.getByRole('tab', { name: /bluesky/i }))
    await waitFor(() =>
      expect(screen.getByRole('tab', { name: /bluesky/i })).toHaveAttribute('aria-selected', 'true')
    )
    expect(screen.getByRole('tab', { name: /x/i })).toHaveAttribute('aria-selected', 'false')
  })
})

// ── Platform tabs derived from siblings ───────────────────────────────────────

describe('EditPostView — tabs from siblings', () => {
  it('shows tabs for all siblings even when post.platforms only lists the current platform', () => {
    const xPost = makeDraft({ platforms: ['x'], platform: 'x', text: 'X text' })
    const bskyPost = makeDraft({ platforms: ['bluesky'], platform: 'bluesky', text: 'Bluesky text' })
    const mastodonPost = makeDraft({ platforms: ['mastodon'], platform: 'mastodon', text: 'Mastodon text' })
    mockCtx.mockReturnValue({ drafts: [xPost, bskyPost, mastodonPost], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    renderEdit({ post: bskyPost })
    expect(screen.getByRole('tab', { name: /x/i })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /bluesky/i })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /mastodon/i })).toBeInTheDocument()
  })
})

// ── Textarea height matches image cap ─────────────────────────────────────────

describe('EditPostView — textarea height', () => {
  it('textarea has an explicit height matching the image cap so it does not over-extend', () => {
    renderEdit()
    const textarea = screen.getByRole('textbox', { name: /post content/i })
    expect(textarea).toHaveStyle({ height: '220px' })
  })
})

// ── Column headers ─────────────────────────────────────────────────────────────

describe('EditPostView — column headers', () => {
  it('shows a Draft header in the draft column', () => {
    renderEdit()
    expect(screen.getByText('Draft')).toBeInTheDocument()
  })

  it('shows a Preview header in the preview column', () => {
    renderEdit()
    const preview = screen.getByTestId('preview-column')
    expect(preview).toContainElement(screen.getByText('Preview'))
  })
})

// ── Char count in preview column ───────────────────────────────────────────────

describe('EditPostView — char count location', () => {
  it('char count is displayed inside the preview column', () => {
    renderEdit({ post: makeDraft({ platform: 'x', text: 'a'.repeat(10) }) })
    const preview = screen.getByTestId('preview-column')
    expect(preview).toHaveTextContent('10 / 280')
  })
})


// ── Approve single-platform behaviour ─────────────────────────────────────────

describe('EditPostView — Approve single platform', () => {
  const xPost = makeDraft({ platforms: ['x', 'bluesky'], platform: 'x', text: 'X text' })
  const bskyPost = makeDraft({ platforms: ['x', 'bluesky'], platform: 'bluesky', text: 'Bluesky text' })

  beforeEach(() => {
    mockCtx.mockReturnValue({ drafts: [xPost, bskyPost], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'approve_post') return undefined; return null })
  })

  it('stays on view and switches to first remaining tab after approving one platform', async () => {
    const onApproved = vi.fn()
    renderEdit({ post: xPost, onApproved })
    fireEvent.click(screen.getByRole('button', { name: 'Approve' }))
    await waitFor(() =>
      expect(screen.getByRole('tab', { name: /bluesky/i })).toHaveAttribute('aria-selected', 'true')
    )
    expect(onApproved).not.toHaveBeenCalled()
  })

  it('calls onApproved when the last remaining platform is approved', async () => {
    const onApproved = vi.fn()
    mockCtx.mockReturnValue({ drafts: [xPost], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    renderEdit({ post: makeDraft(), onApproved })
    fireEvent.click(screen.getByRole('button', { name: /approve/i }))
    await waitFor(() => expect(onApproved).toHaveBeenCalled())
  })
})


void DEFAULT_VIEW
