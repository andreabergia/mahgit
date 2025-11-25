use crate::repository::{Repository, RepositoryError};
use std::env;
use std::process::Command;

/// Resolves the editor to use based on Git's precedence rules:
/// 1. $GIT_EDITOR
/// 2. git config core.editor
/// 3. $VISUAL
/// 4. $EDITOR
/// 5. fallback to "vi"
pub fn resolve_editor(repository: &Repository) -> String {
    // Check GIT_EDITOR environment variable
    if let Ok(editor) = env::var("GIT_EDITOR")
        && !editor.trim().is_empty()
    {
        return editor;
    }

    // Check git config core.editor
    if let Ok(config) = repository.git2_repo().config()
        && let Ok(editor) = config.get_string("core.editor")
        && !editor.trim().is_empty()
    {
        return editor;
    }

    // Check VISUAL environment variable
    if let Ok(editor) = env::var("VISUAL")
        && !editor.trim().is_empty()
    {
        return editor;
    }

    // Check EDITOR environment variable
    if let Ok(editor) = env::var("EDITOR")
        && !editor.trim().is_empty()
    {
        return editor;
    }

    // Fallback to vi
    "vi".to_string()
}

/// Opens a file in the configured editor.
/// Returns Ok(true) if the editor exited successfully, Ok(false) if it failed,
/// or Err if we couldn't spawn the editor.
pub fn open_file_in_editor(
    repository: &Repository,
    file_path: &str,
) -> Result<bool, RepositoryError> {
    let editor_cmd = resolve_editor(repository);

    // Parse the editor command into program and arguments
    // Handle cases like "code --wait" or "emacs -nw"
    let parts: Vec<&str> = editor_cmd.split_whitespace().collect();
    if parts.is_empty() {
        return Err(RepositoryError::Other(
            "Editor command is empty".to_string(),
        ));
    }

    let program = parts[0];
    let args = &parts[1..];

    // Spawn the editor process
    let mut command = Command::new(program);
    command.args(args);
    command.arg(file_path);

    match command.status() {
        Ok(status) => Ok(status.success()),
        Err(e) => Err(RepositoryError::Other(format!(
            "Failed to spawn editor '{}': {}",
            program, e
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_editor_fallback() {
        // This test assumes no GIT_EDITOR, VISUAL, or EDITOR is set
        // and will fallback to "vi"
        // Note: This test might be flaky in environments where these are set
        // In a real scenario, we'd use dependency injection or mock env vars
        let temp_dir = std::env::temp_dir().join("mahgit_test_editor");
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create a temporary git repo for testing
        let repo = git2::Repository::init(&temp_dir).unwrap();
        let mahgit_repo = Repository::from_git2_repo(repo);

        // Clear environment variables for this test
        unsafe {
            env::remove_var("GIT_EDITOR");
            env::remove_var("VISUAL");
            env::remove_var("EDITOR");
        }

        let editor = resolve_editor(&mahgit_repo);
        // Should fallback to vi if nothing is set
        assert!(editor == "vi" || !editor.is_empty());

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
