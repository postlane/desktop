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
  it('shows post_folder label on first row of a group', () => {
    const drafts = [
      makeDraft({ repo_path: '/repo1', post_folder: 'my-post', platform: 'x' }),
      makeDraft({ repo_path: '/repo1', post_folder: 'my-post', platform: 'linkedin' }),
    ]
    render(<PostTable posts={drafts} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getAllByTestId('group-label')).toHaveLength(1)
    expect(screen.getByTestId('group-label')).toHaveTextContent('my-post')
  })

  it('applies background tint to grouped rows', () => {
    const drafts = [
      makeDraft({ repo_path: '/repo1', post_folder: 'my-post', platform: 'x' }),
      makeDraft({ repo_path: '/repo1', post_folder: 'my-post', platform: 'linkedin' }),
    ]
    render(<PostTable posts={drafts} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    const rows = screen.getAllByTestId('post-row')
    rows.forEach((row) => expect(row).toHaveClass('has-background-white-ter'))
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

describe('PostTable — queue mode — time display', () => {
  it('shows scheduled_for as "Scheduled for ..." when present', () => {
    const draft = makeDraft({ scheduled_for: '2024-06-03T09:00:00Z' })
    render(<PostTable posts={[draft]} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByText(/Scheduled for/)).toBeInTheDocument()
  })

  it('shows relative time when scheduled_for is null', () => {
    const now = new Date()
    const twoHoursAgo = new Date(now.getTime() - 2 * 60 * 60 * 1000).toISOString()
    const draft = makeDraft({ scheduled_for: null, created_at: twoHoursAgo })
    render(<PostTable posts={[draft]} isHistory={false} onSelect={vi.fn()} timezone="UTC" />)
    expect(screen.getByText(/ago/)).toBeInTheDocument()
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
})
