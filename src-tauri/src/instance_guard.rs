// SPDX-License-Identifier: BUSL-1.1

/// Checks if another instance is already running by probing `~/.postlane/port`.
/// Returns `Err` if a responsive instance is found, `Ok` if not.
pub fn check_single_instance() -> Result<(), String> {
    let port_path = crate::init::postlane_dir()?.join("port");

    if !port_path.exists() {
        return Ok(());
    }

    let port_str = std::fs::read_to_string(&port_path)
        .map_err(|e| format!("Failed to read port file: {}", e))?;
    let port: u16 = port_str
        .trim()
        .parse()
        .map_err(|e| format!("Invalid port in port file: {}", e))?;

    let url = format!("http://127.0.0.1:{}/health", port);
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create runtime: {}", e))?;
    let health_check = rt.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(200))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Health check failed: {}", e))
    });

    match health_check {
        Ok(_) => Err(format!(
            "Postlane is already running on port {}. Close the existing instance first.",
            port
        )),
        Err(_) => {
            log::warn!("Stale port file detected, cleaning up");
            let _ = std::fs::remove_file(&port_path);
            Ok(())
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn show_alert_and_exit(message: &str) {
    eprintln!("{}", message);
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
fn escape_for_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
pub fn show_alert_and_exit(message: &str) {
    use std::process::Command;
    let _ = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "display dialog \"{}\" buttons {{\"OK\"}} default button \"OK\" with icon caution",
            escape_for_applescript(message)
        ))
        .output();
    std::process::exit(1);
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn test_escape_for_applescript_escapes_double_quotes() {
        let input = r#"Error: "cannot parse" config"#;
        let escaped = escape_for_applescript(input);
        assert_eq!(escaped, r#"Error: \"cannot parse\" config"#);
    }

    #[test]
    fn test_escape_for_applescript_escapes_backslashes_before_quotes() {
        let input = r"path\to\file";
        let escaped = escape_for_applescript(input);
        assert_eq!(escaped, r"path\\to\\file");
    }

    #[test]
    fn test_escape_for_applescript_passthrough_for_plain_text() {
        let input =
            "Postlane is already running on port 9123. Close the existing instance first.";
        let escaped = escape_for_applescript(input);
        assert_eq!(escaped, input);
    }
}
