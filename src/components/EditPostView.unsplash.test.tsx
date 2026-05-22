// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import type { DraftPost, Project } from '../types'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))
vi.mock('../context/DraftPostsProvider', () => ({ useDraftPostsContext: vi.fn() }))

import { invoke } from '../ipc/invoke'
import { useDraftPostsContext } from '../context/DraftPostsProvider'
import EditPostView, { type EditPostViewProps } from './EditPostView'

const mockInvoke = vi.mocked(invoke)
const mockCtx = vi.mocked(useDraftPostsContext)

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
  mockCtx.mockReturnValue({ drafts: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() })
  mockInvoke.mockResolvedValue(null)
})

// ── Unsplash URL blocking in the custom URL field ─────────────────────────────

describe('EditPostView — Unsplash URL blocking in custom URL field', () => {
  it('shows blocking message when images.unsplash.com URL is typed', () => {
    renderEdit({ post: makeDraft({ image_url: null }) })
    fireEvent.change(screen.getByRole('textbox', { name: /add an image from a url/i }), {
      target: { value: 'https://images.unsplash.com/photo-abc' },
    })
    expect(screen.getByText(/use the "search unsplash" above/i)).toBeInTheDocument()
  })

  it('shows blocking message when plus.unsplash.com URL is typed', () => {
    renderEdit({ post: makeDraft({ image_url: null }) })
    fireEvent.change(screen.getByRole('textbox', { name: /add an image from a url/i }), {
      target: { value: 'https://plus.unsplash.com/photo-abc' },
    })
    expect(screen.getByText(/use the "search unsplash" above/i)).toBeInTheDocument()
  })

  it('does not invoke update_post_image when Set image is clicked with an Unsplash URL', async () => {
    renderEdit({ post: makeDraft({ image_url: null }) })
    fireEvent.change(screen.getByRole('textbox', { name: /add an image from a url/i }), {
      target: { value: 'https://images.unsplash.com/photo-abc' },
    })
    fireEvent.click(screen.getByTestId('set-custom-image'))
    await new Promise((r) => setTimeout(r, 50))
    const imageCalls = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'update_post_image')
    expect(imageCalls).toHaveLength(0)
  })

  it('does not show blocking message for a normal image URL', () => {
    renderEdit({ post: makeDraft({ image_url: null }) })
    fireEvent.change(screen.getByRole('textbox', { name: /add an image from a url/i }), {
      target: { value: 'https://example.com/photo.jpg' },
    })
    expect(screen.queryByText(/use the "search unsplash" above/i)).not.toBeInTheDocument()
  })
})

// ── Unsplash attribution display ───────────────────────────────────────────────

describe('EditPostView — Unsplash attribution display', () => {
  it('shows "Photo by [Name] on Unsplash" when post has image attribution', () => {
    renderEdit({ post: makeDraft({
      image_url: 'https://images.unsplash.com/photo-abc',
      image_attribution: { photographer_name: 'Jane Doe', photographer_url: 'https://unsplash.com/@janedoe' },
    }) })
    expect(screen.getByRole('link', { name: 'Jane Doe' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Unsplash' })).toHaveAttribute('href', 'https://unsplash.com')
  })

  it('does not show attribution when post image has no attribution', () => {
    renderEdit({ post: makeDraft({ image_url: 'https://example.com/img.png' }) })
    expect(screen.queryByText(/photo by/i)).not.toBeInTheDocument()
  })

  it('photographer name is a link to their Unsplash profile', () => {
    renderEdit({ post: makeDraft({
      image_url: 'https://images.unsplash.com/photo-abc',
      image_attribution: { photographer_name: 'Jane Doe', photographer_url: 'https://unsplash.com/@janedoe' },
    }) })
    expect(screen.getByRole('link', { name: 'Jane Doe' })).toHaveAttribute('href', 'https://unsplash.com/@janedoe')
  })
})

// ── Unsplash select wiring ────────────────────────────────────────────────────

const UNSPLASH_PHOTO = {
  id: 'abc123',
  description: 'A nature photo',
  urls: {
    raw: 'https://images.unsplash.com/photo-abc?raw',
    full: 'https://images.unsplash.com/photo-abc?full',
    regular: 'https://images.unsplash.com/photo-abc',
    small: 'https://images.unsplash.com/photo-abc?small',
    thumb: 'https://images.unsplash.com/photo-abc?thumb',
  },
  links: { download_location: 'https://api.unsplash.com/photos/abc123/download' },
  user: { name: 'Jane Doe', links: { html: 'https://unsplash.com/@janedoe' } },
}

async function selectUnsplashPhoto() {
  const input = screen.getByRole('searchbox', { name: /search unsplash/i })
  fireEvent.change(input, { target: { value: 'nature' } })
  fireEvent.click(screen.getByRole('button', { name: /search images/i }))
  fireEvent.click(await screen.findByAltText('A nature photo'))
}

describe('EditPostView — Unsplash select wiring', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'search_unsplash') return [UNSPLASH_PHOTO]
      if (cmd === 'update_post_image_unsplash') return null
      if (cmd === 'trigger_unsplash_download') return null
      return null
    })
  })

  it('calls update_post_image_unsplash with all fields when a photo is selected', async () => {
    renderEdit({ post: makeDraft({ image_url: null }) })
    await selectUnsplashPhoto()
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_image_unsplash', {
        repoPath: '/repo1',
        postFolder: 'post-001',
        imageUrl: 'https://images.unsplash.com/photo-abc',
        downloadLocation: 'https://api.unsplash.com/photos/abc123/download',
        photographerName: 'Jane Doe',
        photographerUrl: 'https://unsplash.com/@janedoe',
      })
    )
  })

  it('fires trigger_unsplash_download with download_location when a photo is selected', async () => {
    renderEdit({ post: makeDraft({ image_url: null }) })
    await selectUnsplashPhoto()
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('trigger_unsplash_download', {
        downloadLocation: 'https://api.unsplash.com/photos/abc123/download',
      })
    )
  })

  it('shows attribution in the image section after selecting from Unsplash', async () => {
    renderEdit({ post: makeDraft({ image_url: null }) })
    await selectUnsplashPhoto()
    expect(await screen.findByRole('link', { name: 'Jane Doe' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Unsplash' })).toHaveAttribute('href', 'https://unsplash.com')
  })

  it('does not call update_post_image when an Unsplash photo is selected', async () => {
    renderEdit({ post: makeDraft({ image_url: null }) })
    await selectUnsplashPhoto()
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_image_unsplash', expect.anything())
    )
    const updateImageCalls = mockInvoke.mock.calls.filter(
      ([cmd, args]) => cmd === 'update_post_image' && (args as Record<string, unknown>)?.imageUrl != null
    )
    expect(updateImageCalls).toHaveLength(0)
  })
})

// ── refresh() after image operations ─────────────────────────────────────────

describe('EditPostView — image handlers call refresh', () => {
  let mockRefresh = vi.fn()

  beforeEach(() => {
    mockRefresh = vi.fn()
    mockCtx.mockReturnValue({ drafts: [], loading: false, error: null, refresh: mockRefresh, clear: vi.fn() })
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'search_unsplash') return [UNSPLASH_PHOTO]
      if (cmd === 'update_post_image_unsplash') return null
      if (cmd === 'trigger_unsplash_download') return null
      if (cmd === 'update_post_image') return null
      return null
    })
  })

  it('calls refresh after selecting an Unsplash photo so attribution is persisted in context', async () => {
    renderEdit({ post: makeDraft({ image_url: null }) })
    await selectUnsplashPhoto()
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_image_unsplash', expect.anything())
    )
    expect(mockRefresh).toHaveBeenCalled()
  })

  it('calls refresh after setting a custom image URL', async () => {
    renderEdit({ post: makeDraft({ image_url: null }) })
    fireEvent.change(screen.getByRole('textbox', { name: /add an image from a url/i }), { target: { value: 'https://example.com/img.png' } })
    fireEvent.click(screen.getByTestId('set-custom-image'))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('validate_url_safe', expect.anything())
    )
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_image', expect.objectContaining({ imageUrl: 'https://example.com/img.png' }))
    )
    expect(mockRefresh).toHaveBeenCalled()
  })

  it('auto-saves post text when an Unsplash photo is selected so attribution is not lost on navigate-back', async () => {
    renderEdit({ post: makeDraft({ image_url: null, text: 'My post text' }) })
    await selectUnsplashPhoto()
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_image_unsplash', expect.anything())
    )
    expect(mockInvoke).toHaveBeenCalledWith('save_post_draft', expect.objectContaining({ text: 'My post text' }))
  })

  it('calls refresh after removing an image', async () => {
    renderEdit({ post: makeDraft({ image_url: 'https://example.com/img.png' }) })
    fireEvent.click(screen.getByRole('button', { name: /remove image/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_post_image', expect.objectContaining({ imageUrl: null }))
    )
    expect(mockRefresh).toHaveBeenCalled()
  })
})

