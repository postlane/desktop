// SPDX-License-Identifier: BUSL-1.1

use std::path::Path;

/// Writes `content` to `path` with 0600 permissions on Unix.
/// On non-Unix platforms, falls back to a plain `std::fs::write`.
pub(crate) fn write_secure_file(path: &Path, content: &[u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;

        file.write_all(content)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;

        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)
            .map_err(|e| format!("Failed to set permissions on {}: {}", path.display(), e))?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(path, content)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn write_secure_file_writes_correct_content() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("test.token");
        write_secure_file(&path, b"hello-token").expect("write failed");
        let content = fs::read_to_string(&path).expect("read failed");
        assert_eq!(content, "hello-token");
    }

    #[test]
    #[cfg(unix)]
    fn write_secure_file_sets_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("test.token");
        write_secure_file(&path, b"secret").expect("write failed");
        let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "file must have 0600 permissions");
    }

    #[test]
    fn write_secure_file_returns_error_on_bad_path() {
        let path = Path::new("/nonexistent-dir/cannot-write/file.token");
        let result = write_secure_file(path, b"data");
        assert!(result.is_err(), "must fail on unwritable path");
        let msg = result.expect_err("checked above");
        assert!(
            msg.contains("/nonexistent-dir/cannot-write/file.token"),
            "error must include the path: {msg}",
        );
    }
}
