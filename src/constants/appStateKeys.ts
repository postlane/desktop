// SPDX-License-Identifier: BUSL-1.1

// Key used to persist org expand/collapse state in app_state.json.
// Must match the key read and written by useNavPersistence.
// Defined here once so divergence between read and write sites is a compile error, not a silent reset.
export const NAV_EXPANDED_ORGS_KEY = 'nav_expanded_orgs' as const;
