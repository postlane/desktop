// SPDX-License-Identifier: BUSL-1.1

// TypeScript mirrors of Rust constants in platform_constants.rs.
// Both sites must reference these — never inline the string literals.
// If the Rust constant is renamed, update here too (the compiler won't catch cross-language drift).

/** Emitted by `delete_project` (Rust) when a project is deleted.
 *  `ProjectsProvider` subscribes to trigger a nav refresh. */
export const PROJECTS_CHANGED_EVENT = 'projects-changed' as const;

/** Emitted by `repo_mgmt.rs` watcher when a post meta.json changes.
 *  `DraftPostsProvider` subscribes to reload drafts on each detection. */
export const DRAFT_DETECTED_EVENT = 'meta-changed' as const;
