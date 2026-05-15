// SPDX-License-Identifier: BUSL-1.1

/// Max length for a project ID. Long IDs cause unbounded memory in URL construction.
const MAX_PROJECT_ID_LEN: usize = 64;

pub(crate) fn validate_project_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("project_id must not be empty".to_string());
    }
    if id.len() > MAX_PROJECT_ID_LEN {
        return Err(format!(
            "project_id is too long ({} characters; max {})",
            id.len(),
            MAX_PROJECT_ID_LEN
        ));
    }
    if !id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(format!(
            "project_id '{}' contains invalid characters (only a-z, A-Z, 0-9, hyphens, underscores allowed)",
            id
        ));
    }
    Ok(())
}

pub(crate) fn reject_if_symlink(path: &std::path::Path) -> Result<(), String> {
    match path.symlink_metadata() {
        Ok(m) if m.file_type().is_symlink() => Err(format!(
            "'{}' is a symlink — refusing to read/write to prevent path traversal",
            path.display()
        )),
        Ok(_) | Err(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_project_id_accepts_alphanumeric_and_hyphens() {
        assert!(validate_project_id("proj-abc-123").is_ok());
        assert!(validate_project_id("abc").is_ok());
        assert!(validate_project_id("proj_123-ABC").is_ok());
    }

    #[test]
    fn test_validate_project_id_rejects_empty() {
        let result = validate_project_id("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_validate_project_id_rejects_slash() {
        let result = validate_project_id("proj/../../etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid"));
    }

    #[test]
    fn test_validate_project_id_rejects_spaces() {
        assert!(validate_project_id("proj name").is_err());
    }

    #[test]
    fn test_validate_project_id_rejects_dot_sequences() {
        assert!(validate_project_id("..").is_err());
        assert!(validate_project_id("../other").is_err());
    }

    #[test]
    fn test_validate_project_id_rejects_overlong() {
        let long_id = "a".repeat(MAX_PROJECT_ID_LEN + 1);
        let result = validate_project_id(&long_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too long"), "error must mention too long");
    }

    #[test]
    fn test_validate_project_id_accepts_max_length() {
        let max_id = "a".repeat(MAX_PROJECT_ID_LEN);
        assert!(validate_project_id(&max_id).is_ok());
    }
}
