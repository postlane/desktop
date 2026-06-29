// SPDX-License-Identifier: BUSL-1.1

pub const STABLE: &str =
    "https://github.com/postlane/desktop/releases/latest/download/latest.json";
pub const BETA: &str =
    "https://github.com/postlane/desktop/releases/download/beta/latest.json";

// Emitted by build.rs from TAURI_UPDATER_ENDPOINT; defaults to STABLE.
pub const ENDPOINT: &str = env!("UPDATER_ENDPOINT");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_uses_https() {
        assert!(ENDPOINT.starts_with("https://"), "ENDPOINT must use https://");
    }

    #[test]
    fn endpoint_points_to_latest_json() {
        assert!(
            ENDPOINT.ends_with("/latest.json"),
            "ENDPOINT must point to latest.json"
        );
    }

    #[test]
    fn endpoint_targets_exactly_one_channel() {
        let is_stable = ENDPOINT.contains("/releases/latest/download/");
        let is_beta = ENDPOINT.contains("/releases/download/beta/");
        assert!(
            is_stable ^ is_beta,
            "ENDPOINT must target exactly one channel (stable XOR beta)"
        );
    }

    #[test]
    fn stable_uses_https() {
        assert!(STABLE.starts_with("https://"), "STABLE must use https://");
    }

    #[test]
    fn stable_points_to_releases_latest() {
        assert!(
            STABLE.contains("/releases/latest/download/"),
            "STABLE must point to the /releases/latest/download/ path"
        );
    }

    #[test]
    fn stable_ends_with_latest_json() {
        assert!(
            STABLE.ends_with("/latest.json"),
            "STABLE must end with /latest.json"
        );
    }

    #[test]
    fn beta_uses_https() {
        assert!(BETA.starts_with("https://"), "BETA must use https://");
    }

    #[test]
    fn beta_points_to_beta_channel() {
        assert!(
            BETA.contains("/releases/download/beta/"),
            "BETA must point to the /releases/download/beta/ path"
        );
    }

    #[test]
    fn beta_ends_with_latest_json() {
        assert!(
            BETA.ends_with("/latest.json"),
            "BETA must end with /latest.json"
        );
    }
}
