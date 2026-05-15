// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import '@testing-library/jest-dom'
import type { DraftPost, PublishedPost } from '../types'
import PostTable from './PostTable'

function makeDraft(overrides: Partial<DraftPost> = {}): DraftPost {
  return {
    repo_id: 'r1', repo_name: 'MyRepo', repo_path: '/repo1',
    post_folder: 'post-001', platforms: ['x'], platform: 'x',
    text: 'Hello', status: 'ready', trigger: null, error: null,
    image_url: null, project_id: 'proj-1', schedule: null,
    platform_results: null, llm_model: null, created_at: '2024-06-01T10:00:00Z',
    scheduled_for: null,
    ...overrides,
  }
}

function makePublished(overrides: Partial<PublishedPost> = {}): PublishedPost {
  return {
    repo_id: 'r1', repo_name: 'MyRepo', repo_path: '/repo1',
    post_folder: 'post-001', platforms: ['x'], platform: 'x',
    status: 'sent', scheduler_ids: {}, platform_urls: {},
    provider: null, sent_at: '2024-06-01T10:00:00Z',
    schedule: null, platform_results: null, llm_model: null, created_at: null,
    project_id: 'proj-1',
    ...overrides,
  }
}

beforeEach(() => { vi.clearAllMocks() })

// ── Queue mode ────────────────────────────────────────────────────────────────

describe('PostTable — queue mode — one row per platform', () => {
  it('renders one row per draft post', () => {
    const drafts = [
      makeDraft({ platform: 'x' }),
      makeDraft({ platform: 'linkedin' }),
      makeDraft({ platform: 'bluesky' }),
    ]
    render(<PostTable posts={drafts} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getAllByTestId('post-row')).toHaveLength(3)
  })
})

describe('PostTable — queue mode — visual grouping', () => {
  it('shows one group-label per post group', () => {
    const drafts = [
      makeDraft({ repo_path: '/repo1', post_folder: 'my-post', platform: 'x' }),
      makeDraft({ repo_path: '/repo1', post_folder: 'my-post', platform: 'linkedin' }),
    ]
    render(<PostTable posts={drafts} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getAllByTestId('group-label')).toHaveLength(1)
  })

  it('shows trigger text as group label when trigger is set', () => {
    const drafts = [
      makeDraft({ repo_path: '/repo1', post_folder: 'my-post', platform: 'x', trigger: 'Added new feature' }),
      makeDraft({ repo_path: '/repo1', post_folder: 'my-post', platform: 'linkedin', trigger: 'Added new feature' }),
    ]
    render(<PostTable posts={drafts} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByTestId('group-label')).toHaveTextContent('Added new feature')
  })

  it('falls back to post_folder when trigger is null', () => {
    const drafts = [
      makeDraft({ repo_path: '/repo1', post_folder: 'my-post', platform: 'x', trigger: null }),
    ]
    render(<PostTable posts={drafts} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByTestId('group-label')).toHaveTextContent('my-post')
  })

  it('renders one clickable badge per platform inside the group', () => {
    const drafts = [
      makeDraft({ repo_path: '/repo1', post_folder: 'my-post', platform: 'x' }),
      makeDraft({ repo_path: '/repo1', post_folder: 'my-post', platform: 'linkedin' }),
    ]
    render(<PostTable posts={drafts} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getAllByTestId('post-row')).toHaveLength(2)
  })

  it('creates separate groups for different (repo_path, post_folder) pairs', () => {
    const drafts = [
      makeDraft({ repo_path: '/repo1', post_folder: 'post-001', platform: 'x' }),
      makeDraft({ repo_path: '/repo2', post_folder: 'post-001', platform: 'x' }),
    ]
    render(<PostTable posts={drafts} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getAllByTestId('group-label')).toHaveLength(2)
  })
})

describe('PostTable — queue mode — badge layout', () => {
  it('shows each platform badge with the platform label', () => {
    const drafts = [
      makeDraft({ platform: 'x' }),
      makeDraft({ platform: 'bluesky' }),
    ]
    render(<PostTable posts={drafts} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByRole('button', { name: /edit x post/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /edit bluesky post/i })).toBeInTheDocument()
  })
})

describe('PostTable — queue mode — interactions', () => {
  it('calls onSelect with the post when a row is clicked', () => {
    const onSelect = vi.fn()
    const draft = makeDraft()
    render(<PostTable posts={[draft]} isHistory={false} onSelect={onSelect} timezone="UTC" />)
    fireEvent.click(screen.getByTestId('post-row'))
    expect(onSelect).toHaveBeenCalledWith(draft)
  })

  it('shows danger indicator for failed posts', () => {
    const draft = makeDraft({ status: 'failed' })
    render(<PostTable posts={[draft]} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByTestId('post-row')).toHaveClass('has-text-danger')
  })
})

describe('PostTable — queue mode — empty state', () => {
  it('shows empty queue message when no posts', () => {
    render(<PostTable posts={[]} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByText(/Queue is empty/)).toBeInTheDocument()
    expect(screen.getByText(/draft-post/)).toBeInTheDocument()
  })
})

// ── History mode ──────────────────────────────────────────────────────────────

describe('PostTable — history mode', () => {
  it('renders one row per published post', () => {
    const posts = [
      makePublished({ platform: 'x', sent_at: '2024-06-01T10:00:00Z' }),
      makePublished({ platform: 'linkedin', sent_at: '2024-06-01T09:00:00Z' }),
    ]
    render(<PostTable posts={posts} isHistory={true} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getAllByTestId('post-row')).toHaveLength(2)
  })

  it('shows empty history message when no posts', () => {
    render(<PostTable posts={[]} isHistory={true} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByText(/No posts sent yet/)).toBeInTheDocument()
  })

  it('calls onSelect with the post when a history row is clicked', () => {
    const onSelect = vi.fn()
    const post = makePublished()
    render(<PostTable posts={[post]} isHistory={true} onSelect={onSelect} timezone="UTC" />)
    fireEvent.click(screen.getByTestId('post-row'))
    expect(onSelect).toHaveBeenCalledWith(post)
  })

  it('calls onSelect when Enter is pressed on a history row', () => {
    const onSelect = vi.fn()
    const post = makePublished()
    render(<PostTable posts={[post]} isHistory={true} onSelect={onSelect} timezone="UTC" />)
    fireEvent.keyDown(screen.getByTestId('post-row'), { key: 'Enter' })
    expect(onSelect).toHaveBeenCalledWith(post)
  })

  it('does not call onSelect when a non-Enter key is pressed on a history row', () => {
    const onSelect = vi.fn()
    const post = makePublished()
    render(<PostTable posts={[post]} isHistory={true} onSelect={onSelect} timezone="UTC" />)
    fireEvent.keyDown(screen.getByTestId('post-row'), { key: 'Space' })
    expect(onSelect).not.toHaveBeenCalled()
  })

  it('shows formatted scheduled time when sent_at is null', () => {
    const post = makePublished({ sent_at: null, schedule: '2024-06-10T15:00:00Z' })
    render(<PostTable posts={[post]} isHistory={true} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByTestId('post-row')).toBeInTheDocument()
  })

  it('uses unknown platform label when platform is not in PLATFORM_CFG', () => {
    const post = makePublished({ platform: 'unknown-platform-xyz' })
    render(<PostTable posts={[post]} isHistory={true} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByTestId('post-row')).toBeInTheDocument()
  })

  it('renders when platform is null', () => {
    const post = makePublished({ platform: null })
    render(<PostTable posts={[post]} isHistory={true} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByTestId('post-row')).toBeInTheDocument()
  })

  it('renders without error when sent_at is null', () => {
    const post = makePublished({ sent_at: null })
    render(<PostTable posts={[post]} isHistory={true} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByTestId('post-row')).toBeInTheDocument()
  })
})

describe('PostTable — queue mode — unknown platform', () => {
  it('falls back to platform string for badge label when platform is not in PLATFORM_CFG', () => {
    const draft = makeDraft({ platform: 'unknown-platform-xyz' })
    render(<PostTable posts={[draft]} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByRole('button', { name: /edit unknown-platform-xyz post/i })).toBeInTheDocument()
  })
})
