// SPDX-License-Identifier: BUSL-1.1

import type { Platform } from '../types/index';

export const PLATFORM_LABELS: Record<string, string> = {
  twitter: 'X',
  x: 'X',
  bluesky: 'Bluesky',
  mastodon: 'Mastodon',
  linkedin: 'LinkedIn',
  substack_notes: 'Substack Notes',
  substack: 'Substack',
  product_hunt: 'Product Hunt',
  show_hn: 'Show HN',
  changelog: 'Changelog',
};

export const PLATFORM_ORDER: Platform[] = [
  'x', 'bluesky', 'mastodon', 'linkedin',
  'substack_notes', 'substack', 'product_hunt', 'show_hn', 'changelog',
];

/** Platforms that Zernio can schedule. Others require manual posting. */
export const ZERNIO_SCHEDULABLE_PLATFORMS: ReadonlySet<string> = new Set([
  'x', 'bluesky', 'mastodon', 'linkedin', 'substack_notes',
]);

/** Returns true when the given platform can be auto-scheduled via Zernio. */
export function isSchedulableByZernio(platform: string): boolean {
  return ZERNIO_SCHEDULABLE_PLATFORMS.has(platform);
}
