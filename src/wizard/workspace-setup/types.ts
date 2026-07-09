// SPDX-License-Identifier: BUSL-1.1

// Mirrors src-tauri/src/child_repo_discovery.rs's ChildRepo and
// src-tauri/src/workspace_setup.rs's WorkspaceConfig field-for-field --
// Tauri IPC serializes struct fields as-is (no rename_all), so these must
// stay snake_case to match.

export interface ChildRepo {
  name: string;
  path: string;
  posts_dir: string;
}

export interface WorkspaceConfig {
  project_id: string;
  base_url: string | null;
  platforms: string[];
  mastodon_instance: string | null;
  llm_provider: string;
  llm_model: string;
  author: string;
  style: string;
  utm_campaign: string | null;
  /** `true` = default (append attribution), `false` = user opted out. */
  attribution: boolean;
  scheduler_provider: string;
  scheduler_api_key: string;
  scheduler_profile_id: string | null;
}
