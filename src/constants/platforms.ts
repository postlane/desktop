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
