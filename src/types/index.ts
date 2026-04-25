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
  /** Scheduler provider from .postlane/config.json, or null if not configured */
  provider: string | null;
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
  /** IANA timezone identifier. Empty string = system timezone. */
  timezone: string;
  telemetry_consent: boolean;
  consent_asked: boolean;
}

/** Mirrors analytics::PostAnalytics */
export interface PostAnalytics {
  sessions: number;
  unique_sessions: number;
  top_referrer: string | null;
}

/** Mirrors providers::scheduling::SchedulerProfile */
export interface SchedulerProfile {
  id: string;
  name: string;
  platforms: string[];
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

/** PostMeta enriched with repo context — returned by get_all_drafts */
export interface DraftPost {
  repo_id: string;
  repo_name: string;
  repo_path: string;
  post_folder: string;
  status: 'ready' | 'failed';
  platforms: string[];
  schedule: string | null;
  trigger: string | null;
  platform_results: Record<string, string> | null;
  error: string | null;
  image_url: string | null;
  llm_model: string | null;
  created_at: string | null;
}

export type Platform = 'x' | 'bluesky' | 'mastodon' | 'linkedin' | 'substack_notes' | 'substack' | 'product_hunt' | 'show_hn' | 'changelog';

/** Model edit-rate statistics — returned by get_model_stats */
export interface ModelStats {
  model: string;
  total_posts: number;
  edited_posts: number;
  edit_rate: number;         // 0.0–1.0
  limited_data: boolean;     // true when 5–19 posts
}

/** Sent or queued post with repo context — returned by get_repo_published */
export interface PublishedPost {
  repo_id: string;
  repo_name: string;
  repo_path: string;
  post_folder: string;
  status: 'sent' | 'queued';
  platforms: string[];
  platform_results: Record<string, string> | null;
  schedule: string | null;
  scheduler_ids: Record<string, string> | null;
  platform_urls: Record<string, string> | null;
  /** Scheduler provider name (e.g. "zernio") from repo config.json, or null */
  provider: string | null;
  llm_model: string | null;
  sent_at: string | null;
  created_at: string | null;
}
