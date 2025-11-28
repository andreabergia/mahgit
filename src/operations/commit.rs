use crate::repository::{Repository, RepositoryError};
use crate::ui::input::CommitMode;
use git2::Oid;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Flags that control how a commit is executed
#[derive(Debug, Clone, Default)]
pub struct CommitFlags {
    /// Skip pre-commit and commit-msg hooks
    pub no_verify: bool,
    /// Amend the previous commit
    pub amend: bool,
    /// Skip editing the commit message (use as-is)
    pub no_edit: bool,
}

/// Prepared commit information ready for execution
#[derive(Debug, Clone)]
pub struct CommitPreparation {
    /// The commit mode being used
    pub mode: CommitMode,
    /// Template message for the editor (or final message if no_edit)
    pub message_template: String,
    /// Flags controlling commit behavior
    pub flags: CommitFlags,
}

impl CommitPreparation {
    /// Creates a temporary file with the commit message template
    /// Returns the path to the temporary file
    pub fn create_message_file(&self) -> Result<PathBuf, RepositoryError> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let temp_dir = std::env::temp_dir();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp_file = temp_dir.join(format!(
            "mahgit-commit-{}-{}.txt",
            std::process::id(),
            timestamp
        ));

        let mut file = fs::File::create(&temp_file).map_err(RepositoryError::IoError)?;

        file.write_all(self.message_template.as_bytes())
            .map_err(RepositoryError::IoError)?;

        Ok(temp_file)
    }

    /// Reads the commit message from a file
    pub fn read_message_from_file(path: &PathBuf) -> Result<String, RepositoryError> {
        let content = fs::read_to_string(path).map_err(RepositoryError::IoError)?;

        // Remove comment lines (lines starting with #) and trim
        let message: Vec<&str> = content
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect();

        let message = message.join("\n").trim().to_string();

        if message.is_empty() {
            return Err(RepositoryError::Other(
                "Aborting commit due to empty commit message".to_string(),
            ));
        }

        Ok(message)
    }
}

/// Operations for creating commits
pub struct CommitOperations<'repo> {
    repository: &'repo Repository,
}

impl<'repo> CommitOperations<'repo> {
    pub fn new(repository: &'repo Repository) -> Self {
        Self { repository }
    }

    /// Prepares a commit based on the specified mode
    pub fn prepare_commit(&self, mode: CommitMode) -> Result<CommitPreparation, RepositoryError> {
        match mode {
            CommitMode::Normal => self.prepare_normal_commit(),
            CommitMode::Amend => self.prepare_amend_commit(),
            CommitMode::Extend => self.prepare_extend_commit(),
            CommitMode::Reword => self.prepare_reword_commit(),
        }
    }

    /// Prepares a normal commit with an empty message template
    fn prepare_normal_commit(&self) -> Result<CommitPreparation, RepositoryError> {
        let message_template = self.get_commit_template()?;

        Ok(CommitPreparation {
            mode: CommitMode::Normal,
            message_template,
            flags: CommitFlags::default(),
        })
    }

    /// Prepares an amend commit with the HEAD commit message
    fn prepare_amend_commit(&self) -> Result<CommitPreparation, RepositoryError> {
        let head_message = self.get_head_commit_message()?;

        Ok(CommitPreparation {
            mode: CommitMode::Amend,
            message_template: head_message,
            flags: CommitFlags {
                amend: true,
                ..Default::default()
            },
        })
    }

    /// Prepares an extend commit (amend without editing message)
    fn prepare_extend_commit(&self) -> Result<CommitPreparation, RepositoryError> {
        let head_message = self.get_head_commit_message()?;

        Ok(CommitPreparation {
            mode: CommitMode::Extend,
            message_template: head_message,
            flags: CommitFlags {
                amend: true,
                no_edit: true,
                ..Default::default()
            },
        })
    }

    /// Prepares a reword commit (amend message only, no content changes)
    fn prepare_reword_commit(&self) -> Result<CommitPreparation, RepositoryError> {
        let head_message = self.get_head_commit_message()?;

        Ok(CommitPreparation {
            mode: CommitMode::Reword,
            message_template: head_message,
            flags: CommitFlags {
                amend: true,
                ..Default::default()
            },
        })
    }

    /// Gets the commit message template from git config or returns empty with helpful comments
    fn get_commit_template(&self) -> Result<String, RepositoryError> {
        let git_repo = self.repository.git2_repo();

        // Try to get template from git config
        if let Ok(config) = git_repo.config()
            && let Ok(template_path) = config.get_path("commit.template")
            && let Ok(template) = fs::read_to_string(&template_path)
        {
            return Ok(template);
        }

        // Return default template with helpful comments
        Ok(String::from(
            "\n\
# Please enter the commit message for your changes. Lines starting\n\
# with '#' will be ignored, and an empty message aborts the commit.\n\
#\n",
        ))
    }

    /// Gets the commit message from the HEAD commit
    fn get_head_commit_message(&self) -> Result<String, RepositoryError> {
        let git_repo = self.repository.git2_repo();

        let head = git_repo
            .head()
            .map_err(|e| RepositoryError::Other(format!("Failed to get HEAD: {}", e)))?;

        let commit = head
            .peel_to_commit()
            .map_err(|e| RepositoryError::Other(format!("Failed to get HEAD commit: {}", e)))?;

        let message = commit.message().unwrap_or("");

        Ok(message.to_string())
    }

    /// Executes a commit with the given message
    pub fn execute_commit(
        &self,
        message: &str,
        flags: &CommitFlags,
    ) -> Result<Oid, RepositoryError> {
        let git_repo = self.repository.git2_repo();

        // Get the signature for the commit
        let signature = git_repo
            .signature()
            .map_err(|e| RepositoryError::Other(format!("Failed to get signature: {}", e)))?;

        // Get the tree from the index
        let mut index = git_repo
            .index()
            .map_err(|e| RepositoryError::Other(format!("Failed to get index: {}", e)))?;

        let tree_oid = index
            .write_tree()
            .map_err(|e| RepositoryError::Other(format!("Failed to write tree: {}", e)))?;

        let tree = git_repo
            .find_tree(tree_oid)
            .map_err(|e| RepositoryError::Other(format!("Failed to find tree: {}", e)))?;

        // Determine parent commits
        let parents = if flags.amend {
            // For amend, use the parents of HEAD (skipping HEAD itself)
            self.get_amend_parents()?
        } else {
            // For normal commit, HEAD is the parent (if it exists)
            self.get_normal_parents()?
        };

        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        // TODO: Hook support - run pre-commit and commit-msg hooks unless no_verify is set
        // This would involve:
        // 1. Check if hooks exist
        // 2. Execute them with appropriate arguments
        // 3. Handle their exit codes

        // Create the commit
        let commit_oid = git_repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )
            .map_err(|e| RepositoryError::Other(format!("Failed to create commit: {}", e)))?;

        Ok(commit_oid)
    }

    /// Gets the parent commits for a normal commit
    fn get_normal_parents(&self) -> Result<Vec<git2::Commit<'_>>, RepositoryError> {
        let git_repo = self.repository.git2_repo();

        match git_repo.head() {
            Ok(head) => {
                let commit = head.peel_to_commit().map_err(|e| {
                    RepositoryError::Other(format!("Failed to get HEAD commit: {}", e))
                })?;
                Ok(vec![commit])
            }
            Err(e) => {
                // If HEAD doesn't exist, this is the first commit (no parents)
                if e.code() == git2::ErrorCode::UnbornBranch {
                    Ok(vec![])
                } else {
                    Err(RepositoryError::Other(format!("Failed to get HEAD: {}", e)))
                }
            }
        }
    }

    /// Gets the parent commits for an amend commit (parents of HEAD)
    fn get_amend_parents(&self) -> Result<Vec<git2::Commit<'_>>, RepositoryError> {
        let git_repo = self.repository.git2_repo();

        let head = git_repo
            .head()
            .map_err(|e| RepositoryError::Other(format!("Failed to get HEAD: {}", e)))?;

        let head_commit = head
            .peel_to_commit()
            .map_err(|e| RepositoryError::Other(format!("Failed to get HEAD commit: {}", e)))?;

        // Get the parents of HEAD
        let mut parents = Vec::new();
        for i in 0..head_commit.parent_count() {
            let parent = head_commit.parent(i).map_err(|e| {
                RepositoryError::Other(format!("Failed to get parent {}: {}", i, e))
            })?;
            parents.push(parent);
        }

        Ok(parents)
    }

    /// Checks if there are staged changes ready to commit
    pub fn has_staged_changes(&self) -> Result<bool, RepositoryError> {
        let git_repo = self.repository.git2_repo();

        // Get the current index
        let mut index = git_repo
            .index()
            .map_err(|e| RepositoryError::Other(format!("Failed to get index: {}", e)))?;

        // Check if HEAD exists (for initial commit case)
        let head_tree = match git_repo.head() {
            Ok(head) => {
                let commit = head.peel_to_commit().map_err(|e| {
                    RepositoryError::Other(format!("Failed to get HEAD commit: {}", e))
                })?;
                Some(commit.tree().map_err(|e| {
                    RepositoryError::Other(format!("Failed to get HEAD tree: {}", e))
                })?)
            }
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                // First commit - no HEAD tree
                None
            }
            Err(e) => {
                return Err(RepositoryError::Other(format!("Failed to get HEAD: {}", e)));
            }
        };

        // If HEAD exists, compare index with HEAD tree
        if let Some(head_tree) = head_tree {
            let index_tree_oid = index.write_tree_to(git_repo).map_err(|e| {
                RepositoryError::Other(format!("Failed to write index tree: {}", e))
            })?;

            // If the tree OIDs are different, there are staged changes
            Ok(index_tree_oid != head_tree.id())
        } else {
            // No HEAD (first commit) - check if index has any entries
            Ok(!index.is_empty())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::Repository;
    use std::fs;

    fn create_test_repo(_name: &str) -> (Repository, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path();

        let git_repo = git2::Repository::init(repo_path).unwrap();

        // Configure user for commits
        let mut config = git_repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        let repository = Repository::from_git2_repo(git_repo);

        (repository, temp_dir)
    }

    #[test]
    fn test_prepare_normal_commit() {
        let (repo, _temp_dir) = create_test_repo("normal_commit");
        let commit_ops = CommitOperations::new(&repo);

        let preparation = commit_ops.prepare_normal_commit().unwrap();

        assert!(matches!(preparation.mode, CommitMode::Normal));
        assert!(!preparation.flags.amend);
        assert!(!preparation.flags.no_edit);
        assert!(!preparation.flags.no_verify);
        assert!(
            preparation
                .message_template
                .contains("Please enter the commit message")
        );
    }

    #[test]
    fn test_has_staged_changes_empty_repo() {
        let (repo, _temp_dir) = create_test_repo("empty");
        let commit_ops = CommitOperations::new(&repo);

        // Empty repo with nothing staged should return false
        assert!(!commit_ops.has_staged_changes().unwrap());
    }

    #[test]
    fn test_has_staged_changes_with_file() {
        let (repo, temp_dir) = create_test_repo("staged");

        // Create and stage a file
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);

        // Should have staged changes now
        assert!(commit_ops.has_staged_changes().unwrap());
    }

    #[test]
    fn test_execute_commit() {
        let (repo, temp_dir) = create_test_repo("execute");

        // Create and stage a file
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);
        let flags = CommitFlags::default();

        // Execute the commit
        let commit_oid = commit_ops
            .execute_commit("Test commit message", &flags)
            .unwrap();

        // Verify the commit was created
        let git_repo = repo.git2_repo();
        let commit = git_repo.find_commit(commit_oid).unwrap();
        assert_eq!(commit.message().unwrap(), "Test commit message");
    }

    #[test]
    fn test_create_and_read_message_file() {
        let preparation = CommitPreparation {
            mode: CommitMode::Normal,
            message_template: "Test message\n# Comment line\nMore content".to_string(),
            flags: CommitFlags::default(),
        };

        let temp_file = preparation.create_message_file().unwrap();
        assert!(temp_file.exists());

        let message = CommitPreparation::read_message_from_file(&temp_file).unwrap();
        // Note: lines() removes newlines, so filtered lines are joined with single \n
        assert_eq!(message, "Test message\nMore content");

        // Cleanup
        fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_read_message_rejects_empty() {
        let preparation = CommitPreparation {
            mode: CommitMode::Normal,
            message_template: "# Only comments\n# More comments".to_string(),
            flags: CommitFlags::default(),
        };

        let temp_file = preparation.create_message_file().unwrap();

        let result = CommitPreparation::read_message_from_file(&temp_file);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("empty commit message")
        );

        // Cleanup
        fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_prepare_amend_commit() {
        let (repo, temp_dir) = create_test_repo("amend");

        // Create an initial commit
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "initial content").unwrap();
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);
        commit_ops
            .execute_commit("Initial commit", &CommitFlags::default())
            .unwrap();

        // Prepare amend commit
        let preparation = commit_ops.prepare_amend_commit().unwrap();

        assert!(matches!(preparation.mode, CommitMode::Amend));
        assert!(preparation.flags.amend);
        assert_eq!(preparation.message_template, "Initial commit");
    }

    #[test]
    fn test_prepare_extend_commit() {
        let (repo, temp_dir) = create_test_repo("extend");

        // Create an initial commit
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "initial content").unwrap();
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);
        commit_ops
            .execute_commit("Initial commit", &CommitFlags::default())
            .unwrap();

        // Prepare extend commit
        let preparation = commit_ops.prepare_extend_commit().unwrap();

        assert!(matches!(preparation.mode, CommitMode::Extend));
        assert!(preparation.flags.amend);
        assert!(preparation.flags.no_edit);
        assert_eq!(preparation.message_template, "Initial commit");
    }
}
