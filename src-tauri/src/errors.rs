// SPDX-License-Identifier: BUSL-1.1

//! Structured error types for the Postlane desktop backend.
//!
//! All variants convert to `String` via `From<PostlaneError> for String`,
//! making them usable with `?` in functions that return `Result<T, String>`
//! (the type Tauri requires for command return values).

use std::fmt;

#[derive(Debug)]
pub enum PostlaneError {
    /// An internal `Mutex` was poisoned. The `&'static str` names the lock
    /// (e.g. `"repos"`, `"state"`) so the message is diagnosable.
    MutexPoisoned(&'static str),
    /// A repository path failed validation (not registered, invalid symlink, etc.).
    Repo(String),
}

impl fmt::Display for PostlaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MutexPoisoned(name) => {
                write!(f, "Internal lock '{name}' was poisoned — restart the app")
            }
            Self::Repo(msg) => f.write_str(msg),
        }
    }
}

impl From<PostlaneError> for String {
    fn from(e: PostlaneError) -> Self {
        e.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutex_poisoned_message_names_the_lock() {
        let msg = PostlaneError::MutexPoisoned("repos").to_string();
        assert!(msg.contains("repos"), "must name the lock, got: {msg}");
    }

    #[test]
    fn test_mutex_poisoned_message_indicates_lock_failure() {
        let msg = PostlaneError::MutexPoisoned("repos").to_string();
        let lower = msg.to_lowercase();
        assert!(
            lower.contains("lock") || lower.contains("poison"),
            "must describe a lock failure, got: {msg}",
        );
    }

    #[test]
    fn test_repo_error_preserves_message() {
        let msg = PostlaneError::Repo("path '/foo' not registered".to_string()).to_string();
        assert!(msg.contains("/foo"), "must preserve message content, got: {msg}");
    }

    #[test]
    fn test_from_postlane_error_for_string_roundtrips() {
        let s: String = PostlaneError::MutexPoisoned("watchers").into();
        assert!(!s.is_empty());
        assert!(s.contains("watchers"), "String conversion must retain lock name, got: {s}");
    }

    #[test]
    fn test_different_lock_names_produce_different_messages() {
        let a = PostlaneError::MutexPoisoned("repos").to_string();
        let b = PostlaneError::MutexPoisoned("state").to_string();
        assert_ne!(a, b, "different lock names must produce different messages");
    }
}
