// SPDX-License-Identifier: BUSL-1.1

use tauri::State;
use crate::app_state::AppState;

/// Stub -- cancel is not yet implemented.
#[tauri::command]
pub fn cancel_post_command(
    _repo_path: String,
    _post_folder: String,
    _post_id: String,
    _platform: String,
    _state: State<AppState>,
) -> Result<(), String> {
    cancel_post_impl()
}

pub(crate) fn cancel_post_impl() -> Result<(), String> {
    Err("Post cancellation is not yet available. To unschedule, use your scheduler dashboard.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancel_post_impl_returns_err() {
        assert!(cancel_post_impl().is_err(), "cancel must return Err");
    }

    #[test]
    fn test_cancel_post_impl_message_contains_not_yet_available() {
        let msg = cancel_post_impl().unwrap_err();
        assert!(
            msg.contains("not yet available"),
            "frontend filter depends on 'not yet available', got: {}", msg
        );
    }

    #[test]
    fn test_cancel_post_impl_message_references_scheduler_dashboard() {
        let msg = cancel_post_impl().unwrap_err();
        assert!(
            msg.contains("scheduler dashboard") || msg.contains("dashboard"),
            "must tell user to use dashboard, got: {}", msg
        );
    }

    #[test]
    fn test_cancel_post_impl_has_no_em_dash() {
        let msg = cancel_post_impl().unwrap_err();
        assert!(
            !msg.contains('\u{2014}'),
            "em dashes banned per project rules, got: {}", msg
        );
    }

    #[test]
    fn test_cancel_post_impl_has_no_internal_language() {
        let msg = cancel_post_impl().unwrap_err();
        assert!(!msg.contains("Milestone"), "must not leak internal roadmap: {}", msg);
        assert!(!msg.contains("M4"), "must not leak internal roadmap: {}", msg);
        assert!(!msg.contains("deferred"), "must not leak internal roadmap: {}", msg);
        assert!(!msg.contains("delete the draft"), "wrong guidance for queued posts: {}", msg);
    }
}
