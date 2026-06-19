use git2::Repository as Git2Repository;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub enum RepositoryError {
    NotFound,
    NotARepository,
    Corrupted(String),
    AccessDenied,
    GitError(git2::Error),
    IoError(std::io::Error),
    Other(String),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryError::NotFound => write!(f, "Not in a Git repository"),
            RepositoryError::NotARepository => write!(f, "Not a Git repository"),
            RepositoryError::Corrupted(msg) => write!(f, "Repository corrupted: {}", msg),
            RepositoryError::AccessDenied => write!(f, "Access denied to repository"),
            RepositoryError::GitError(e) => write!(f, "Git error: {}", e),
            RepositoryError::IoError(e) => write!(f, "IO error: {}", e),
            RepositoryError::Other(msg) => write!(f, "Repository error: {}", msg),
        }
    }
}

impl std::error::Error for RepositoryError {}

pub struct Repository {
    git_repo: Git2Repository,
}

impl Repository {
    pub fn discover<P: AsRef<Path>>(path: P) -> Result<Self, RepositoryError> {
        let git_repo = Git2Repository::discover(path).map_err(|e| match e.code() {
            git2::ErrorCode::NotFound => RepositoryError::NotFound,
            _ => RepositoryError::Other(e.message().to_string()),
        })?;

        Ok(Repository { git_repo })
    }

    /// Create a Repository from an existing Git2Repository (useful for tests)
    #[cfg(test)]
    pub fn from_git2_repo(git_repo: Git2Repository) -> Self {
        Repository { git_repo }
    }

    pub fn current_branch_name(&self) -> Result<String, RepositoryError> {
        let head = self
            .git_repo
            .head()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        if let Some(name) = head.shorthand() {
            Ok(name.to_string())
        } else {
            Ok("HEAD".to_string())
        }
    }

    pub fn get_statuses(&self) -> Result<git2::Statuses<'_>, RepositoryError> {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        opts.renames_head_to_index(true);
        opts.renames_index_to_workdir(true);

        self.git_repo
            .statuses(Some(&mut opts))
            .map_err(|e| RepositoryError::Other(e.message().to_string()))
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, RepositoryError> {
        let git_repo = Git2Repository::open(path).map_err(|e| match e.code() {
            git2::ErrorCode::NotFound => RepositoryError::NotFound,
            _ => RepositoryError::Other(e.message().to_string()),
        })?;

        Ok(Repository { git_repo })
    }

    pub fn get_index(&self) -> Result<git2::Index, RepositoryError> {
        self.git_repo
            .index()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))
    }

    pub fn add_to_index(&self, path: &str) -> Result<(), RepositoryError> {
        let mut index = self.get_index()?;
        let normalized_path = Self::normalize_repo_relative_path_allow_empty(path)?;

        // Check if the file exists in the working directory relative to the repo root
        let workdir = self.git_repo.workdir().ok_or_else(|| {
            RepositoryError::Other("Repository has no working directory".to_string())
        })?;
        let absolute_file_path = workdir.join(&normalized_path);

        if !absolute_file_path.exists() {
            // File is missing in working directory. Handle tracked files as deletions.
            let is_in_index = index.get_path(&normalized_path, 0).is_some();
            let is_tracked_in_head = self.is_tracked_path(&normalized_path)?;

            if is_in_index {
                // Remove it from the index to stage the deletion
                index
                    .remove_path(&normalized_path)
                    .map_err(|e| RepositoryError::Other(e.message().to_string()))?;
            } else if is_tracked_in_head {
                // For deleted tracked directories, remove nested entries from index.
                // If nothing matches, deletion was already staged.
                self.remove_path_prefix_from_index(&mut index, &normalized_path)?;
            } else {
                // File doesn't exist in working directory and was never tracked
                return Err(RepositoryError::Other(format!(
                    "File '{}' does not exist and is not tracked",
                    path
                )));
            }
        } else if absolute_file_path.is_dir() {
            // If it's a directory, recursively add all files within it
            self.add_directory_to_index(&absolute_file_path, workdir, &mut index)?;
        } else {
            // File exists, add it normally
            index
                .add_path(&normalized_path)
                .map_err(|e| RepositoryError::Other(e.message().to_string()))?;
        }

        index
            .write()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;
        Ok(())
    }

    fn add_directory_to_index(
        &self,
        dir_path: &std::path::Path,
        workdir: &std::path::Path,
        index: &mut git2::Index,
    ) -> Result<(), RepositoryError> {
        // Walk through all entries in the directory recursively
        let git_dir = workdir.join(".git");
        let walker = walkdir::WalkDir::new(dir_path)
            .into_iter()
            .filter_entry(|entry| {
                entry.path() != git_dir.as_path() && !entry.path().starts_with(&git_dir)
            })
            .filter_map(|entry| entry.ok());

        for entry in walker {
            if entry.file_type().is_file() {
                // Get the relative path from the working directory
                let relative_path = entry.path().strip_prefix(workdir).map_err(|e| {
                    RepositoryError::Other(format!("Failed to compute relative path: {}", e))
                })?;

                index
                    .add_path(relative_path)
                    .map_err(|e| RepositoryError::Other(e.message().to_string()))?;
            }
        }

        Ok(())
    }

    fn remove_path_prefix_from_index(
        &self,
        index: &mut git2::Index,
        path: &Path,
    ) -> Result<(), RepositoryError> {
        let path_str = path.to_string_lossy();
        let dir_prefix = format!("{}/", path_str);
        let paths_to_remove: Vec<PathBuf> = index
            .iter()
            .filter_map(|entry| {
                let entry_path = String::from_utf8_lossy(&entry.path);
                if entry_path == path_str || entry_path.starts_with(&dir_prefix) {
                    Some(PathBuf::from(entry_path.as_ref()))
                } else {
                    None
                }
            })
            .collect();

        for entry_path in paths_to_remove {
            index
                .remove_path(&entry_path)
                .map_err(|e| RepositoryError::Other(e.message().to_string()))?;
        }

        Ok(())
    }

    pub fn reset_file(&self, path: &str) -> Result<(), RepositoryError> {
        let head_commit = self
            .git_repo
            .head()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?
            .peel_to_commit()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        // Pass the commit object, not the tree object
        let head_commit_object = head_commit.as_object();

        self.git_repo
            .reset_default(Some(head_commit_object), [path])
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        Ok(())
    }

    pub fn git2_repo(&self) -> &Git2Repository {
        &self.git_repo
    }

    /// Load commits from HEAD, skipping `skip` and returning up to `count`.
    /// Returns the entries and whether more commits exist.
    pub fn load_log_entries(
        &self,
        count: usize,
        skip: usize,
    ) -> Result<(Vec<crate::log::LogEntry>, bool), RepositoryError> {
        let mut revwalk = self
            .git_repo
            .revwalk()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        revwalk
            .push_head()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        revwalk
            .set_sorting(git2::Sort::TIME)
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        let mut entries = Vec::new();
        let mut skipped = 0;
        let mut has_more = false;

        for oid_result in revwalk {
            let oid = oid_result.map_err(|e| RepositoryError::Other(e.message().to_string()))?;

            if skipped < skip {
                skipped += 1;
                continue;
            }

            if entries.len() >= count {
                has_more = true;
                break;
            }

            let commit = self
                .git_repo
                .find_commit(oid)
                .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

            let author = commit.author();
            let committer = commit.committer();
            let short_hash = format!("{:.7}", oid);
            let summary = commit.summary().unwrap_or("").to_string();
            let message = commit.message().unwrap_or("").to_string();
            let author_name = author.name().unwrap_or("Unknown").to_string();
            let author_email = author.email().unwrap_or("").to_string();
            let time = author.when();
            let committer_name = committer.name().unwrap_or("Unknown").to_string();
            let committer_email = committer.email().unwrap_or("").to_string();
            let committer_time = committer.when();

            entries.push(crate::log::LogEntry {
                oid,
                short_hash,
                summary,
                message,
                author_name,
                author_email,
                time,
                committer_name,
                committer_email,
                committer_time,
            });
        }

        Ok((entries, has_more))
    }

    /// Get the list of changed files for a commit (compared to its first parent).
    pub fn changed_files_for_commit(
        &self,
        oid: git2::Oid,
    ) -> Result<Vec<CommitFileChange>, RepositoryError> {
        let commit = self
            .git_repo
            .find_commit(oid)
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        let tree = commit
            .tree()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        let parent_tree = if commit.parent_count() > 0 {
            Some(
                commit
                    .parent(0)
                    .map_err(|e| RepositoryError::Other(e.message().to_string()))?
                    .tree()
                    .map_err(|e| RepositoryError::Other(e.message().to_string()))?,
            )
        } else {
            None
        };

        let diff = self
            .git_repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        let mut changes = Vec::new();
        for delta in diff.deltas() {
            let path = delta
                .new_file()
                .path()
                .and_then(|p| p.to_str())
                .unwrap_or("<invalid>")
                .to_string();
            let old_path = delta
                .old_file()
                .path()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string());
            let change_type = match delta.status() {
                git2::Delta::Added => CommitChangeType::Added,
                git2::Delta::Deleted => CommitChangeType::Deleted,
                git2::Delta::Modified => CommitChangeType::Modified,
                git2::Delta::Renamed => CommitChangeType::Renamed,
                _ => CommitChangeType::Other,
            };
            changes.push(CommitFileChange {
                path,
                old_path,
                change_type,
            });
        }

        Ok(changes)
    }

    /// Check if a file is tracked in the repository (exists in HEAD)
    pub fn is_tracked(&self, path: &str) -> Result<bool, RepositoryError> {
        let normalized_path = Self::normalize_repo_relative_path(path)?;
        self.is_tracked_path(&normalized_path)
    }

    fn is_tracked_path(&self, file_path: &Path) -> Result<bool, RepositoryError> {
        // Get HEAD commit
        let head = match self.git_repo.head() {
            Ok(head) => head,
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                // Repository has no commits yet, so no files are tracked
                return Ok(false);
            }
            Err(e) => {
                return Err(RepositoryError::Other(format!(
                    "Failed to get HEAD: {}",
                    e.message()
                )));
            }
        };

        let commit = head
            .peel_to_commit()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        let tree = commit
            .tree()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        // Check if the path exists in the tree
        Ok(tree.get_path(file_path).is_ok())
    }

    fn normalize_repo_relative_path(path: &str) -> Result<PathBuf, RepositoryError> {
        Self::normalize_repo_relative_path_with_options(path, false)
    }

    fn normalize_repo_relative_path_allow_empty(path: &str) -> Result<PathBuf, RepositoryError> {
        Self::normalize_repo_relative_path_with_options(path, true)
    }

    fn normalize_repo_relative_path_with_options(
        path: &str,
        allow_empty: bool,
    ) -> Result<PathBuf, RepositoryError> {
        let input = Path::new(path);
        if input.is_absolute() {
            return Err(RepositoryError::Other(format!(
                "Path '{}' must be relative to repository root",
                path
            )));
        }

        let mut normalized = PathBuf::new();
        for component in input.components() {
            match component {
                Component::CurDir => {}
                Component::Normal(part) => normalized.push(part),
                Component::ParentDir => {
                    if !normalized.pop() {
                        normalized.push(component.as_os_str());
                    }
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(RepositoryError::Other(format!(
                        "Path '{}' must be relative to repository root",
                        path
                    )));
                }
            }
        }

        if normalized.as_os_str().is_empty() && !allow_empty {
            return Err(RepositoryError::Other("Path cannot be empty".to_string()));
        }

        Ok(normalized)
    }

    /// Discard changes to a file in the working directory
    /// For tracked files: restores the file to the version in the index
    /// For untracked files: deletes the file
    pub fn discard_file(&self, path: &str) -> Result<(), RepositoryError> {
        let is_tracked = self.is_tracked(path)?;

        if is_tracked {
            // For tracked files, use git2's checkout to restore from index
            let mut checkout_builder = git2::build::CheckoutBuilder::new();
            checkout_builder.force().update_only(true).path(path);

            self.git_repo
                .checkout_index(None, Some(&mut checkout_builder))
                .map_err(|e| {
                    RepositoryError::Other(format!("Failed to discard file: {}", e.message()))
                })?;
        } else {
            // For untracked files, delete them
            let workdir = self.git_repo.workdir().ok_or_else(|| {
                RepositoryError::Other("Repository has no working directory".to_string())
            })?;
            let file_path = workdir.join(path);

            std::fs::remove_file(&file_path).map_err(|e| {
                RepositoryError::IoError(std::io::Error::new(
                    e.kind(),
                    format!("Failed to delete untracked file '{}': {}", path, e),
                ))
            })?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CommitFileChange {
    pub path: String,
    pub old_path: Option<String>,
    pub change_type: CommitChangeType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitChangeType {
    Added,
    Deleted,
    Modified,
    Renamed,
    Other,
}

impl std::fmt::Display for CommitChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitChangeType::Added => write!(f, "A"),
            CommitChangeType::Deleted => write!(f, "D"),
            CommitChangeType::Modified => write!(f, "M"),
            CommitChangeType::Renamed => write!(f, "R"),
            CommitChangeType::Other => write!(f, "?"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_error_display() {
        assert_eq!(
            RepositoryError::NotFound.to_string(),
            "Not in a Git repository"
        );
        assert_eq!(
            RepositoryError::NotARepository.to_string(),
            "Not a Git repository"
        );
        assert_eq!(
            RepositoryError::Corrupted("index".to_string()).to_string(),
            "Repository corrupted: index"
        );
        assert_eq!(
            RepositoryError::AccessDenied.to_string(),
            "Access denied to repository"
        );
        assert_eq!(
            RepositoryError::Other("custom error".to_string()).to_string(),
            "Repository error: custom error"
        );

        // Test GitError and IoError variants are handled (without specific messages)
        let git_error = git2::Error::from_str("test git error");
        let repo_error = RepositoryError::GitError(git_error);
        assert!(repo_error.to_string().contains("Git error"));

        let io_error = std::io::Error::other("test io error");
        let repo_error = RepositoryError::IoError(io_error);
        assert!(repo_error.to_string().contains("IO error"));
    }
}
