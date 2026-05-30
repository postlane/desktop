// SPDX-License-Identifier: BUSL-1.1

//! Central registry of all PL-* error codes produced by the desktop app.
//!
//! Every user-visible error code string must have an entry here. The no-duplicate
//! and completeness properties are enforced by `error_registry_tests.rs`.

pub struct ErrorInfo {
    pub code: &'static str,
    pub summary: &'static str,
}

/// All registered error codes. No code may appear twice.
/// Add an entry here whenever you introduce a new PL-* code in production code,
/// AND add a runbook entry in `internal/support/error-runbook.md`.
pub const ERROR_REGISTRY: &[ErrorInfo] = &[
    ErrorInfo { code: "PL-WS-001", summary: "No Git repositories found in selected folder" },
    ErrorInfo { code: "PL-WS-002", summary: "Workspace folder belongs to a different project or config.json unreadable" },
    ErrorInfo { code: "PL-WS-003", summary: "Selected folder is itself a Git repository — select the parent" },
    ErrorInfo { code: "PL-MIG-001", summary: "Post copy verification failed (byte count mismatch)" },
    ErrorInfo { code: "PL-MIG-002", summary: "Migration journal write failed — workspace directory may be read-only" },
    ErrorInfo { code: "PL-DEL-000", summary: "Account deletion pre-flight failed — session expired or no license token" },
    ErrorInfo { code: "PL-DEL-001", summary: "Project API delete returned non-2xx error" },
    ErrorInfo { code: "PL-DEL-002", summary: "Workspace path failed safelist check — too shallow, is $HOME, or not registered" },
    ErrorInfo { code: "PL-DEL-003", summary: "GitLab token revocation skipped — instance URL failed SSRF validation" },
    ErrorInfo { code: "PL-DEL-004", summary: "Supabase account delete failed — network error or server error" },
    ErrorInfo { code: "PL-DEL-005", summary: "Cannot read workspace registry before deletion — repos.json missing or corrupt" },
];

#[cfg(test)]
#[path = "error_registry_tests.rs"]
mod tests;
