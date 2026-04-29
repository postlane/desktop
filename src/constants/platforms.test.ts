// SPDX-License-Identifier: BUSL-1.1

import { describe, expect, it } from 'vitest';
import { PLATFORM_LABELS, PLATFORM_ORDER } from './platforms';

describe('PLATFORM_LABELS', () => {
  it('maps canonical platform keys to display names', () => {
    expect(PLATFORM_LABELS['x']).toBe('X');
    expect(PLATFORM_LABELS['bluesky']).toBe('Bluesky');
    expect(PLATFORM_LABELS['mastodon']).toBe('Mastodon');
    expect(PLATFORM_LABELS['linkedin']).toBe('LinkedIn');
    expect(PLATFORM_LABELS['substack_notes']).toBe('Substack Notes');
    expect(PLATFORM_LABELS['substack']).toBe('Substack');
    expect(PLATFORM_LABELS['product_hunt']).toBe('Product Hunt');
    expect(PLATFORM_LABELS['show_hn']).toBe('Show HN');
    expect(PLATFORM_LABELS['changelog']).toBe('Changelog');
  });

  it('includes twitter alias for backward compatibility', () => {
    expect(PLATFORM_LABELS['twitter']).toBe('X');
  });

  it('returns undefined for unknown keys', () => {
    expect(PLATFORM_LABELS['unknown_platform']).toBeUndefined();
  });
});

describe('PLATFORM_ORDER', () => {
  it('contains all canonical platforms', () => {
    expect(PLATFORM_ORDER).toContain('x');
    expect(PLATFORM_ORDER).toContain('bluesky');
    expect(PLATFORM_ORDER).toContain('mastodon');
    expect(PLATFORM_ORDER).toContain('linkedin');
    expect(PLATFORM_ORDER).toContain('substack_notes');
    expect(PLATFORM_ORDER).toContain('substack');
    expect(PLATFORM_ORDER).toContain('product_hunt');
    expect(PLATFORM_ORDER).toContain('show_hn');
    expect(PLATFORM_ORDER).toContain('changelog');
  });

  it('has x before bluesky', () => {
    expect(PLATFORM_ORDER.indexOf('x')).toBeLessThan(PLATFORM_ORDER.indexOf('bluesky'));
  });
});
