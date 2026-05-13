// SPDX-License-Identifier: BUSL-1.1

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
      timezone="UTC" onBack={vi.fn()} onApproved={vi.fn()} onToast={vi.fn()}
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

// ── Rendering ──────────────────────────────────────────────────────────────────

describe('EditPostView — rendering', () => {
  it('renders textarea in queue mode', () => {
    renderEdit()
    expect(screen.getByRole('textbox', { name: /post content/i })).toBeInTheDocument()
  })

  it('renders read-only div in history mode', () => {
    renderEdit({ post: makePublished(), isHistory: true })
    expect(screen.queryByRole('textbox', { name: /post content/i })).not.toBeInTheDocument()
    expect(screen.getByTestId('post-text')).toBeInTheDocument()
  })

  it('shows error text inline for failed post', () => {
    const draft = makeDraft({ status: 'failed', error: 'Scheduler timeout' })
    renderEdit({ post: draft })
    expect(screen.getByText('Scheduler timeout')).toBeInTheDocument()
  })
})

// ── Approve / Retry button ─────────────────────────────────────────────────────

describe('EditPostView — Approve disabled: over limit', () => {
  it('disables Approve with char-limit tooltip when text exceeds platform limit', () => {
    const draft = makeDraft({ platform: 'x', text: 'a'.repeat(281) })
    renderEdit({ post: draft })
    const btn = screen.getByRole('button', { name: /Approve/i })
    expect(btn).toBeDisabled()
    expect(btn).toHaveAttribute('title', expect.stringContaining('character limit'))
  })
})

describe('EditPostView — Approve disabled: billing inactive', () => {
  it('disables Approve with billing tooltip when project billing is inactive', () => {
    renderEdit({ project: makeProject({ billing_active: false }) })
    const btn = screen.getByRole('button', { name: /Approve/i })
    expect(btn).toBeDisabled()
    expect(btn).toHaveAttribute('title', expect.stringContaining('Billing inactive'))
  })
})

describe('EditPostView — Approve disabled: dirty', () => {
  it('disables Approve with dirty tooltip when text has unsaved changes', () => {
    renderEdit()
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed text' } })
    const btn = screen.getByRole('button', { name: /Approve/i })
    expect(btn).toBeDisabled()
    expect(btn).toHaveAttribute('title', 'Save your changes before approving.')
  })
})

describe('EditPostView — Approve success', () => {
  it('calls refresh, onApproved, and onToast when Approve succeeds', async () => {
    const onApproved = vi.fn()
    const onToast = vi.fn()
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'approve_post') return undefined; return null })
    renderEdit({ onApproved, onToast })
    fireEvent.click(screen.getByRole('button', { name: /Approve/i }))
    await waitFor(() => expect(onApproved).toHaveBeenCalled())
    expect(mockRefresh).toHaveBeenCalled()
    expect(onToast).toHaveBeenCalledWith('Post approved.', 3000)
  })
})

describe('EditPostView — failed post shows Retry', () => {
  it('shows Retry button instead of Approve for failed post', () => {
    renderEdit({ post: makeDraft({ status: 'failed', error: 'Error' }) })
    expect(screen.getByRole('button', { name: /Retry/i })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^Approve$/i })).not.toBeInTheDocument()
  })
})

// ── Save button ────────────────────────────────────────────────────────────────

describe('EditPostView — Save', () => {
  it('Save button has warning background when text is dirty', () => {
    renderEdit()
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed' } })
    expect(screen.getByRole('button', { name: /Save/i })).toHaveClass('has-background-warning')
  })

  it('calls save_post_draft with correct args', async () => {
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'save_post_draft') return undefined; return null })
    const draft = makeDraft({ repo_path: '/repo1', post_folder: 'post-001', platform: 'x', text: 'original' })
    renderEdit({ post: draft })
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'updated text' } })
    fireEvent.click(screen.getByRole('button', { name: /Save/i }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('save_post_draft', {
      repoPath: '/repo1', postFolder: 'post-001', platform: 'x', text: 'updated text',
    }))
  })

  it('Approve is enabled after successful Save', async () => {
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'save_post_draft') return undefined; return null })
    renderEdit()
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed' } })
    fireEvent.click(screen.getByRole('button', { name: /Save/i }))
    await waitFor(() => expect(screen.getByRole('button', { name: /Approve/i })).not.toBeDisabled())
  })

  it('shows error when save_post_draft fails', async () => {
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'save_post_draft') throw new Error('disk full'); return null })
    renderEdit()
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed' } })
    fireEvent.click(screen.getByRole('button', { name: /Save/i }))
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
  })
})

// ── Delete ─────────────────────────────────────────────────────────────────────

describe('EditPostView — Delete', () => {
  it('shows confirmation dialog mentioning the platform', () => {
    renderEdit({ post: makeDraft({ platform: 'x' }) })
    fireEvent.click(screen.getByRole('button', { name: /Delete/i }))
    expect(screen.getByText(/Other platforms/i)).toBeInTheDocument()
  })

  it('calls delete_post when delete is confirmed', async () => {
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'delete_post') return undefined; return null })
    renderEdit({ post: makeDraft({ repo_path: '/repo1', post_folder: 'post-001', platform: 'x' }) })
    fireEvent.click(screen.getByRole('button', { name: /Delete/i }))
    fireEvent.click(screen.getByTestId('confirm-delete'))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('delete_post', {
      repoPath: '/repo1', postFolder: 'post-001', platform: 'x',
    }))
  })

  it('calls refresh after successful delete', async () => {
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'delete_post') return undefined; return null })
    renderEdit()
    fireEvent.click(screen.getByRole('button', { name: /Delete/i }))
    fireEvent.click(screen.getByTestId('confirm-delete'))
    await waitFor(() => expect(mockRefresh).toHaveBeenCalled())
  })
})

// ── Discard guard ──────────────────────────────────────────────────────────────

describe('EditPostView — Back discard guard', () => {
  it('Back calls onBack immediately when clean', () => {
    const onBack = vi.fn()
    renderEdit({ onBack })
    fireEvent.click(screen.getByRole('button', { name: /Back/i }))
    expect(onBack).toHaveBeenCalled()
  })

  it('Back shows discard prompt when dirty', () => {
    renderEdit()
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed' } })
    fireEvent.click(screen.getByRole('button', { name: /Back/i }))
    expect(screen.getByText(/Discard unsaved changes/i)).toBeInTheDocument()
  })

  it('Discard confirm calls onBack', async () => {
    const onBack = vi.fn()
    renderEdit({ onBack })
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed' } })
    fireEvent.click(screen.getByRole('button', { name: /Back/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Discard$/i }))
    expect(onBack).toHaveBeenCalled()
  })

  it('Cancel hides the discard prompt', () => {
    renderEdit()
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed' } })
    fireEvent.click(screen.getByRole('button', { name: /Back/i }))
    fireEvent.click(screen.getByRole('button', { name: /^Cancel$/i }))
    expect(screen.queryByText(/Discard unsaved changes/i)).not.toBeInTheDocument()
  })
})

describe('EditPostView — LeftNav navigation guard', () => {
  it('shows discard prompt when pendingNavSel is set while dirty', () => {
    const { rerender } = renderEdit({ pendingNavSel: null })
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed' } })
    rerender(
      <EditPostView post={makeDraft()} project={makeProject()} isHistory={false}
        timezone="UTC" onBack={vi.fn()} onApproved={vi.fn()} onToast={vi.fn()}
        onNavigate={vi.fn()} pendingNavSel={DEFAULT_NAV_SEL} onNavCancelled={vi.fn()} />,
    )
    expect(screen.getByText(/Discard unsaved changes/i)).toBeInTheDocument()
  })

  it('calls onNavCancelled when Cancel is clicked in nav discard prompt', () => {
    const onNavCancelled = vi.fn()
    const { rerender } = renderEdit({ pendingNavSel: null, onNavCancelled })
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed' } })
    rerender(
      <EditPostView post={makeDraft()} project={makeProject()} isHistory={false}
        timezone="UTC" onBack={vi.fn()} onApproved={vi.fn()} onToast={vi.fn()}
        onNavigate={vi.fn()} pendingNavSel={DEFAULT_NAV_SEL} onNavCancelled={onNavCancelled} />,
    )
    fireEvent.click(screen.getByRole('button', { name: /^Cancel$/i }))
    expect(onNavCancelled).toHaveBeenCalled()
  })

  it('calls onNavigate with dest when Discard is confirmed for nav guard', () => {
    const onNavigate = vi.fn()
    const { rerender } = renderEdit({ pendingNavSel: null, onNavigate })
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed' } })
    rerender(
      <EditPostView post={makeDraft()} project={makeProject()} isHistory={false}
        timezone="UTC" onBack={vi.fn()} onApproved={vi.fn()} onToast={vi.fn()}
        onNavigate={onNavigate} pendingNavSel={DEFAULT_NAV_SEL} onNavCancelled={vi.fn()} />,
    )
    fireEvent.click(screen.getByRole('button', { name: /^Discard$/i }))
    expect(onNavigate).toHaveBeenCalledWith(DEFAULT_NAV_SEL)
  })
})

// ── History mode ───────────────────────────────────────────────────────────────

describe('EditPostView — history mode', () => {
  it('does not render a textarea in history mode', () => {
    renderEdit({ post: makePublished(), isHistory: true })
    expect(screen.queryByRole('textbox', { name: /post content/i })).not.toBeInTheDocument()
  })

  it('shows Repost button disabled with a title in history mode', () => {
    renderEdit({ post: makePublished(), isHistory: true })
    const btn = screen.getByRole('button', { name: /Repost/i })
    expect(btn).toBeDisabled()
    expect(btn).toHaveAttribute('title')
  })

  it('shows analytics placeholder in history mode', () => {
    renderEdit({ post: makePublished(), isHistory: true })
    expect(screen.getByText(/analytics/i)).toBeInTheDocument()
    expect(screen.getByText(/v2/i)).toBeInTheDocument()
  })
})

// ── Preview ────────────────────────────────────────────────────────────────────

describe('EditPostView — Preview', () => {
  it('opens PreviewModal when Preview is clicked', () => {
    renderEdit()
    fireEvent.click(screen.getByRole('button', { name: /Preview/i }))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })
})

// ── OG image detection ─────────────────────────────────────────────────────────

describe('EditPostView — OG image detection', () => {
  it('calls fetch_og_image after debounce when URL is present in text', async () => {
    vi.useFakeTimers()
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'fetch_og_image') return null; return null })
    const draft = makeDraft({ text: 'Check https://example.com for details' })
    renderEdit({ post: draft })
    await act(async () => { await vi.advanceTimersByTimeAsync(500) })
    expect(mockInvoke).toHaveBeenCalledWith('fetch_og_image', { url: 'https://example.com' })
  })

  it('displays image when fetch_og_image returns a URL', async () => {
    vi.useFakeTimers()
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'fetch_og_image') return 'https://example.com/og.png'
      return null
    })
    const draft = makeDraft({ text: 'See https://example.com for details' })
    renderEdit({ post: draft })
    await act(async () => { await vi.advanceTimersByTimeAsync(500) })
    expect(screen.getByTestId('og-image')).toBeInTheDocument()
  })
})

// ── Custom image URL ───────────────────────────────────────────────────────────

describe('EditPostView — custom image URL', () => {
  it('calls validate_url_safe for a valid https URL', async () => {
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'validate_url_safe') return undefined; return null })
    renderEdit()
    fireEvent.change(screen.getByLabelText(/custom image url/i), { target: { value: 'https://example.com/img.png' } })
    fireEvent.click(screen.getByTestId('set-custom-image'))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('validate_url_safe', { url: 'https://example.com/img.png' }))
  })

  it('shows inline error when validate_url_safe rejects', async () => {
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'validate_url_safe') throw new Error('SSRF'); return null })
    renderEdit()
    fireEvent.change(screen.getByLabelText(/custom image url/i), { target: { value: 'https://internal.corp/img.png' } })
    fireEvent.click(screen.getByTestId('set-custom-image'))
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(mockInvoke).toHaveBeenCalledWith('validate_url_safe', expect.anything())
  })

  it('rejects http:// URL without calling validate_url_safe', async () => {
    renderEdit()
    fireEvent.change(screen.getByLabelText(/custom image url/i), { target: { value: 'http://bad.com/img.png' } })
    fireEvent.click(screen.getByTestId('set-custom-image'))
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(mockInvoke).not.toHaveBeenCalledWith('validate_url_safe', expect.anything())
  })
})

// ── Cmd+Enter shortcut ─────────────────────────────────────────────────────────

describe('EditPostView — Cmd+Enter', () => {
  it('saves when dirty', async () => {
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'save_post_draft') return undefined; return null })
    renderEdit()
    fireEvent.change(screen.getByRole('textbox', { name: /post content/i }), { target: { value: 'changed' } })
    fireEvent.keyDown(document, { key: 'Enter', metaKey: true })
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('save_post_draft', expect.anything()))
  })

  it('approves when clean and within limit', async () => {
    const onApproved = vi.fn()
    mockInvoke.mockImplementation(async (cmd) => { if (cmd === 'approve_post') return undefined; return null })
    renderEdit({ onApproved })
    fireEvent.keyDown(document, { key: 'Enter', metaKey: true })
    await waitFor(() => expect(onApproved).toHaveBeenCalled())
  })
})

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
        timezone="UTC" onBack={vi.fn()} onApproved={vi.fn()} onToast={vi.fn()}
        onNavigate={onNavigate} pendingNavSel={DEFAULT_NAV_SEL} onNavCancelled={vi.fn()} />,
    )
    expect(onNavigate).toHaveBeenCalledWith(DEFAULT_NAV_SEL)
  })
})

// ── Preview close ──────────────────────────────────────────────────────────────

describe('EditPostView — Preview close', () => {
  it('closes PreviewModal when the close button is clicked', async () => {
    renderEdit()
    fireEvent.click(screen.getByRole('button', { name: /Preview/i }))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    const closeBtn = screen.getByRole('button', { name: /close preview/i })
    fireEvent.click(closeBtn)
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })
})
