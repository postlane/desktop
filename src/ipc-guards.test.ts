// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest';
import { isDraftPost, isPublishedPost } from './ipc-guards';

describe('isDraftPost (§review-engineering-low)', () => {
  it('returns true for a valid ready post', () => {
    expect(isDraftPost({
      repo_id: 'r1', repo_name: 'my-app', repo_path: '/p', post_folder: 'f',
      status: 'ready', platforms: ['x'], schedule: null, platform_results: null,
      llm_model: null, created_at: null, trigger: null, error: null, image_url: null,
    })).toBe(true);
  });

  it('returns false when status is "sent" (not a draft)', () => {
    expect(isDraftPost({ repo_id: 'r1', post_folder: 'f', status: 'sent', platforms: [] })).toBe(false);
  });

  it('returns false when repo_id is missing', () => {
    expect(isDraftPost({ post_folder: 'f', status: 'ready', platforms: [] })).toBe(false);
  });

  it('returns false when post_folder is missing', () => {
    expect(isDraftPost({ repo_id: 'r1', status: 'ready', platforms: [] })).toBe(false);
  });

  it('returns false for a non-object', () => {
    expect(isDraftPost(null)).toBe(false);
    expect(isDraftPost('string')).toBe(false);
    expect(isDraftPost(42)).toBe(false);
  });

  it('returns false when platforms is not an array', () => {
    expect(isDraftPost({ repo_id: 'r1', post_folder: 'f', status: 'ready', platforms: 'x' })).toBe(false);
  });
});

describe('isPublishedPost (§review-engineering-low)', () => {
  it('returns true for a valid sent post', () => {
    expect(isPublishedPost({
      repo_id: 'r1', repo_name: 'my-app', repo_path: '/p', post_folder: 'f',
      status: 'sent', platforms: ['x'], schedule: null, platform_results: null,
      llm_model: null, created_at: null, scheduler_ids: null, platform_urls: null,
      provider: null, sent_at: '2026-04-15T10:00:00Z',
    })).toBe(true);
  });

  it('returns true for a queued post', () => {
    expect(isPublishedPost({
      repo_id: 'r1', post_folder: 'f', status: 'queued', platforms: ['x'],
      schedule: null, platform_results: null, llm_model: null, created_at: null,
      scheduler_ids: null, platform_urls: null, provider: null, sent_at: null,
    })).toBe(true);
  });

  it('returns false when status is "ready" (not a published post)', () => {
    expect(isPublishedPost({ repo_id: 'r1', post_folder: 'f', status: 'ready', platforms: [] })).toBe(false);
  });

  it('returns false when repo_id is missing', () => {
    expect(isPublishedPost({ post_folder: 'f', status: 'sent', platforms: [] })).toBe(false);
  });

  it('returns false when post_folder is missing', () => {
    expect(isPublishedPost({ repo_id: 'r1', status: 'sent', platforms: [] })).toBe(false);
  });

  it('returns false for null', () => {
    expect(isPublishedPost(null)).toBe(false);
  });
});
