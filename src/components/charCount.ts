// SPDX-License-Identifier: BUSL-1.1

// Mirrors the count_chars function in parser.rs exactly.
// X and Mastodon replace every URL with a 23-character placeholder before
// counting — matching the t.co wrapping rule used by both platforms.
// Bluesky counts full URL length with no replacement.

const URL_REGEX = /https?:\/\/[^\s]+/g;
const URL_SHORT_LENGTH = 23;
const PLACEHOLDER = 'x'.repeat(URL_SHORT_LENGTH);

/// X: replace all URLs with 23-char placeholder, then count Unicode scalars.
export function countCharsX(content: string): number {
  return [...content.replace(URL_REGEX, PLACEHOLDER)].length;
}

/// Bluesky: full Unicode scalar count, no URL replacement.
export function countCharsBluesky(content: string): number {
  return [...content].length;
}

/// Mastodon: same URL replacement rule as X, Unicode-aware count.
export function countCharsMastodon(content: string): number {
  return [...content.replace(URL_REGEX, PLACEHOLDER)].length;
}

/// LinkedIn: full character count, no URL replacement.
/// LinkedIn counts every character including URLs at their true length.
export function countLinkedInChars(content: string): number {
  return [...content].length;
}
