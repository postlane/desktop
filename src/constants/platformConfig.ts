// SPDX-License-Identifier: BUSL-1.1

// Platform keys here must match KNOWN_SOCIAL_PLATFORMS in src-tauri/src/platform_constants.rs.
// If a platform is added or renamed on the Rust side, update both files together.

/** Character limits per social platform. 0 = no enforced limit. */
export const CHAR_LIMITS: Record<string, number> = {
  x: 280,
  bluesky: 300,
  mastodon: 500,
  linkedin: 3000,
  substack_notes: 280,
  substack: 0,
  product_hunt: 260,
  show_hn: 0,
  changelog: 0,
};

/** Display label and brand colour per social platform. */
export const PLATFORM_CFG: Record<string, { label: string; color: string }> = {
  x:        { label: 'X',        color: 'hsl(0, 0%, 10%)'    },
  linkedin:  { label: 'LinkedIn', color: 'hsl(211, 69%, 40%)' },
  bluesky:   { label: 'Bluesky',  color: 'hsl(211, 80%, 55%)' },
};
