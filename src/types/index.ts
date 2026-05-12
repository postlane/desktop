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
  /** project_id from .postlane/config.json, or null if not linked to a project */
  project_id: string | null;
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
  default_post_time: { hour: number; minute: number; timezone: string } | null;
  notifications_enabled?: boolean;
  dismissed_unassigned_draft_warning?: boolean;
  post_wizard_completed?: boolean;
  org_upgrade_banner_dismissed_v1_2?: boolean;
}

/** Mirrors analytics::PostAnalytics */
export interface PostAnalytics {
  configured: boolean;
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

export type OrgNavView = 'queue' | 'history' | 'settings';
export type GlobalSettingsSection = 'account' | 'preferences' | 'system';

export interface OrgQueueView { view: 'org_queue'; projectId: string }
export interface OrgHistoryView { view: 'org_history'; projectId: string }
export interface OrgSettingsView { view: 'org_settings'; projectId: string; section: OrgNavView }
export interface GlobalSettingsView { view: 'global_settings'; section: GlobalSettingsSection }
export interface NoOrgsView { view: 'no_orgs' }

export type ViewSelection = OrgQueueView | OrgHistoryView | OrgSettingsView | GlobalSettingsView | NoOrgsView;

export interface Project {
  id: string;
  name: string;
  workspace_type: 'personal' | 'organization' | 'client';
  tier: string;
  billing_active: boolean;
  is_owner: boolean;
  /** GitHub org login linked to this project. Null for projects created before v1.2. */
  provider_org_login?: string | null;
}

export type ImageState =
  | { status: 'none' }
  | { status: 'loading' }
  | { status: 'loaded'; url: string }
  | { status: 'error'; message: string }

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

/** Shared fields present on every post regardless of lifecycle stage */
interface PostBase {
  repo_id: string;
  repo_name: string;
  repo_path: string;
  post_folder: string;
  platforms: string[];
  schedule: string | null;
  schedule_source?: string | null;
  schedule_timezone?: string | null;
  platform_results: Record<string, string> | null;
  llm_model: string | null;
  created_at: string | null;
}

/** Draft post — status 'ready' or 'failed' — returned by get_all_drafts */
export interface DraftPost extends PostBase {
  status: 'ready' | 'failed';
  trigger: string | null;
  error: string | null;
  image_url: string | null;
  /** project_id from repo config.json; null for repos added before M19 */
  project_id: string | null;
  /** LLM model that generated the post — populated by rebuilt get_all_drafts (19.0.15) */
  model_name?: string | null;
  /** UTC ISO8601 timestamp the post is scheduled to publish; null if not scheduled */
  scheduled_for?: string | null;
  /** ISO8601 timestamp of the most recent user edit; null if the post has never been edited */
  edited_at?: string | null;
  /** v2 stub: local download path for Unsplash image attached to this post */
  image_download_location?: string | null;
  /** v2 stub: Unsplash photographer attribution required by download endpoint compliance */
  image_attribution?: { photographer_name: string; photographer_url: string } | null;
  /** Single platform key for this row (e.g. "x") — draft rows are one per platform */
  platform: string;
  /** Post body text read from the platform .md file */
  text: string;
}

/** Published/queued post — status 'sent' or 'queued' — returned by get_repo_published */
export interface PublishedPost extends PostBase {
  status: 'sent' | 'queued';
  scheduler_ids: Record<string, string> | null;
  platform_urls: Record<string, string> | null;
  /** Scheduler provider name (e.g. "zernio") from repo config.json, or null */
  provider: string | null;
  sent_at: string | null;
  // M19 fields — populated by get_org_published; optional for backward-compat with get_repo_published
  /** Published post body text — read from the platform .md file */
  text?: string | null;
  /** Single platform key for this row (e.g. "x") — M19 History rows are per-platform */
  platform?: string | null;
  /** project_id from the repo's config.json */
  project_id?: string | null;
}

/** Canonical union type covering both lifecycle stages */
export type Post = DraftPost | PublishedPost;

export type Platform = 'x' | 'bluesky' | 'mastodon' | 'linkedin' | 'substack_notes' | 'substack' | 'product_hunt' | 'show_hn' | 'changelog';

/** Mirrors types::SendResult — returned by approve_post */
export interface SendResult {
  success: boolean;
  platform_results: Record<string, string> | null;
  error: string | null;
  fallback_provider: string | null;
}

/** Global edit-rate aggregate — returned by get_model_stats (M19+) */
export interface ModelStatsResponse {
  edit_rate: number;           // 0.0–1.0; 0.0 when total_posts is 0
  edited_posts: number;
  total_posts: number;
  denominator_unit: string;    // "platform_approval"
  pre_m19_post_count: number;  // approvals from posts where edited_platforms was absent
}
