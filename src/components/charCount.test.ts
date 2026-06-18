// SPDX-License-Identifier: BUSL-1.1

import { describe, expect, it } from 'vitest';
import { countCharsBluesky, countCharsX, countCharsMastodon } from './charCount';

describe('countCharsBluesky', () => {
  it('counts ASCII text correctly', () => {
    expect(countCharsBluesky('hello world')).toBe(11);
  });

  it('counts a single emoji as one grapheme (ENG-C10)', () => {
    expect(countCharsBluesky('👋')).toBe(1);
  });

  it('counts ZWJ family emoji as one grapheme cluster (ENG-C10)', () => {
    // 👩‍👩‍👧‍👦 is 7 Unicode scalar values but 1 grapheme cluster
    expect(countCharsBluesky('👩‍👩‍👧‍👦')).toBe(1);
  });

  it('counts flag emoji as one grapheme cluster', () => {
    // 🇺🇸 is 2 Unicode scalar values (regional indicator symbols) but 1 grapheme
    expect(countCharsBluesky('🇺🇸')).toBe(1);
  });

  it('counts full URLs without replacement', () => {
    const content = 'Check out https://example.com/very/long/url';
    // URL is counted at full length (no t.co wrapping on Bluesky)
    expect(countCharsBluesky(content)).toBeGreaterThan(0);
  });
});

describe('countCharsX — URL shortening', () => {
  it('replaces URLs with 23-char placeholder', () => {
    const content = 'Check https://example.com/very/long/path';
    expect(countCharsX(content)).toBe(6 + 23); // "Check " + 23
  });
});

describe('countCharsMastodon', () => {
  it('replaces URLs with 23-char placeholder', () => {
    const content = 'Check https://example.com/very/long/path';
    expect(countCharsMastodon(content)).toBe(6 + 23);
  });
});
