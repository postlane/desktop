// SPDX-License-Identifier: BUSL-1.1

/** Mirrors storage::Repo + computed fields from get_repos command */
export interface RepoWithStatus {
  id: string;
  name: string;
  path: string;
  active: boolean;
  added_at: string;
  path_exists: boolean;
  ready_count: number;
  failed_count: number;
  /** ISO 8601 timestamp of most recent post created_at, or null */
  last_post_at: string | null;
}

/** Mirrors app_state::NavState */
export interface NavState {
  last_view: string;
  last_repo_id: string | null;
  last_section: string;
  expanded_repos: string[];
}

/** Mirrors app_state::WindowState */
export interface WindowState {
  width: number;
  height: number;
  x: number;
  y: number;
}

/** Mirrors app_state::AppStateFile */
export interface AppStateFile {
  version: number;
  window: WindowState;
  nav: NavState;
  wizard_completed: boolean;
}

export type Section = 'drafts' | 'published';
export type NavView = 'all_repos' | 'repo';

export interface ViewSelection {
  view: NavView;
  repoId: string | null;
  section: Section;
}

export type StatusIndicatorType =
  | { type: 'none' }        // inactive repo — no dot shown
  | { type: 'watching' }    // active repo, watcher running, no pending posts
  | { type: 'warning' }     // path not found
  | { type: 'single'; color: 'red' | 'green' }
  | { type: 'stacked' };

/** Payload from Tauri meta-changed event */
export interface MetaChangedPayload {
  repo_id: string;
  post_folder: string;
}
