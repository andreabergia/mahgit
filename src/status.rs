use crate::repository::{Repository, RepositoryError};
use git2::Status;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Typechange,
}

impl fmt::Display for FileStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status_str = match self {
            FileStatus::Added => "added",
            FileStatus::Modified => "modified",
            FileStatus::Deleted => "deleted",
            FileStatus::Renamed => "renamed",
            FileStatus::Typechange => "typechange",
        };
        write!(f, "{}", status_str)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileEntry {
    pub path: String,
    pub status: FileStatus,
    pub old_path: Option<String>,
}

impl FileEntry {
    pub fn new(path: String, status: FileStatus) -> Self {
        Self {
            path,
            status,
            old_path: None,
        }
    }

    pub fn new_renamed(old_path: String, new_path: String) -> Self {
        Self {
            path: new_path,
            status: FileStatus::Renamed,
            old_path: Some(old_path),
        }
    }
}

// Helper function to expand directories to individual files
fn expand_untracked_directories(untracked: Vec<String>, repo_path: &Path) -> Vec<String> {
    let mut expanded = Vec::new();

    for path_str in untracked {
        let path = repo_path.join(&path_str);

        if path.is_dir() {
            let mut found_files = false;
            for entry in walkdir::WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file()
                    && let Ok(relative_path) = entry.path().strip_prefix(repo_path)
                    && let Some(path_str) = relative_path.to_str()
                {
                    expanded.push(path_str.replace('\\', "/")); // Normalize path separators
                    found_files = true;
                }
            }
            // Fallback: keep the directory entry if we couldn't find any files
            if !found_files {
                expanded.push(path_str);
            }
        } else {
            // If it's a file, just add it as is
            expanded.push(path_str);
        }
    }

    expanded.sort();
    expanded
}

#[derive(Debug, Default)]
pub struct RepositoryStatus {
    pub branch_name: String,
    pub staged: Vec<FileEntry>,
    pub unstaged: Vec<FileEntry>,
    pub untracked: Vec<String>,
    pub conflicted: Vec<String>,
}

impl RepositoryStatus {
    pub fn new(repository: &Repository) -> Result<Self, RepositoryError> {
        let branch_name = repository.current_branch_name()?;
        let statuses = repository.get_statuses()?;

        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();
        let mut conflicted = Vec::new();

        // workdir not needed when using relative paths

        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("<invalid utf-8>").to_string();

            let status = entry.status();

            if status.contains(Status::CONFLICTED) {
                conflicted.push(path.clone());
            } else if status.contains(Status::WT_NEW) {
                untracked.push(path.clone());
            } else {
                // Handle staged changes
                if status.contains(Status::INDEX_NEW) {
                    staged.push(FileEntry::new(path.clone(), FileStatus::Added));
                } else if status.contains(Status::INDEX_MODIFIED) {
                    staged.push(FileEntry::new(path.clone(), FileStatus::Modified));
                } else if status.contains(Status::INDEX_DELETED) {
                    staged.push(FileEntry::new(path.clone(), FileStatus::Deleted));
                } else if status.contains(Status::INDEX_RENAMED) {
                    // Extract both old and new paths from head_to_index delta
                    if let Some(delta) = entry.head_to_index() {
                        let old_path = delta
                            .old_file()
                            .path()
                            .and_then(|p| p.to_str())
                            .unwrap_or("<invalid utf-8>")
                            .to_string();
                        let new_path = delta
                            .new_file()
                            .path()
                            .and_then(|p| p.to_str())
                            .unwrap_or("<invalid utf-8>")
                            .to_string();
                        staged.push(FileEntry::new_renamed(old_path, new_path));
                    } else {
                        // Fallback if delta is not available
                        staged.push(FileEntry::new(path.clone(), FileStatus::Renamed));
                    }
                } else if status.contains(Status::INDEX_TYPECHANGE) {
                    staged.push(FileEntry::new(path.clone(), FileStatus::Typechange));
                }

                // Handle unstaged changes
                if status.contains(Status::WT_MODIFIED) {
                    unstaged.push(FileEntry::new(path.clone(), FileStatus::Modified));
                } else if status.contains(Status::WT_DELETED) {
                    unstaged.push(FileEntry::new(path.clone(), FileStatus::Deleted));
                } else if status.contains(Status::WT_RENAMED) {
                    // Extract both old and new paths from index_to_workdir delta
                    if let Some(delta) = entry.index_to_workdir() {
                        let old_path = delta
                            .old_file()
                            .path()
                            .and_then(|p| p.to_str())
                            .unwrap_or("<invalid utf-8>")
                            .to_string();
                        let new_path = delta
                            .new_file()
                            .path()
                            .and_then(|p| p.to_str())
                            .unwrap_or("<invalid utf-8>")
                            .to_string();
                        unstaged.push(FileEntry::new_renamed(old_path, new_path));
                    } else {
                        // Fallback if delta is not available
                        unstaged.push(FileEntry::new(path.clone(), FileStatus::Renamed));
                    }
                } else if status.contains(Status::WT_TYPECHANGE) {
                    unstaged.push(FileEntry::new(path.clone(), FileStatus::Typechange));
                }
            }
        }

        // Expand any directories in untracked files to show individual files
        let repo_path = repository.git2_repo().workdir().ok_or_else(|| {
            RepositoryError::Other("Repository has no working directory".to_string())
        })?;
        untracked = expand_untracked_directories(untracked, repo_path);

        Ok(RepositoryStatus {
            branch_name,
            staged,
            unstaged,
            untracked,
            conflicted,
        })
    }

    pub fn is_clean(&self) -> bool {
        self.staged.is_empty()
            && self.unstaged.is_empty()
            && self.untracked.is_empty()
            && self.conflicted.is_empty()
    }

    pub fn staged_files(&self) -> &Vec<FileEntry> {
        &self.staged
    }

    pub fn unstaged_files(&self) -> &Vec<FileEntry> {
        &self.unstaged
    }

    pub fn untracked_files(&self) -> &Vec<String> {
        &self.untracked
    }

    pub fn conflicted_files(&self) -> &Vec<String> {
        &self.conflicted
    }

    pub fn empty() -> Self {
        RepositoryStatus {
            branch_name: "main".to_string(),
            staged: Vec::new(),
            unstaged: Vec::new(),
            untracked: Vec::new(),
            conflicted: Vec::new(),
        }
    }

    pub fn reload(&mut self, repository: &Repository) -> Result<(), RepositoryError> {
        let new_status = Self::new(repository)?;
        self.branch_name = new_status.branch_name;
        self.staged = new_status.staged;
        self.unstaged = new_status.unstaged;
        self.untracked = new_status.untracked;
        self.conflicted = new_status.conflicted;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_status_is_clean() {
        let status = RepositoryStatus {
            branch_name: "main".to_string(),
            staged: vec![],
            unstaged: vec![],
            untracked: vec![],
            conflicted: vec![],
        };
        assert!(status.is_clean());
    }

    #[test]
    fn test_repository_status_is_not_clean_with_staged() {
        let status = RepositoryStatus {
            branch_name: "main".to_string(),
            staged: vec![FileEntry::new("file.txt".to_string(), FileStatus::Modified)],
            unstaged: vec![],
            untracked: vec![],
            conflicted: vec![],
        };
        assert!(!status.is_clean());
    }

    #[test]
    fn test_repository_status_is_not_clean_with_unstaged() {
        let status = RepositoryStatus {
            branch_name: "main".to_string(),
            staged: vec![],
            unstaged: vec![FileEntry::new("file.txt".to_string(), FileStatus::Modified)],
            untracked: vec![],
            conflicted: vec![],
        };
        assert!(!status.is_clean());
    }

    #[test]
    fn test_repository_status_is_not_clean_with_untracked() {
        let status = RepositoryStatus {
            branch_name: "main".to_string(),
            staged: vec![],
            unstaged: vec![],
            untracked: vec!["file.txt".to_string()],
            conflicted: vec![],
        };
        assert!(!status.is_clean());
    }

    #[test]
    fn test_repository_status_is_not_clean_with_conflicted() {
        let status = RepositoryStatus {
            branch_name: "main".to_string(),
            staged: vec![],
            unstaged: vec![],
            untracked: vec![],
            conflicted: vec!["file.txt".to_string()],
        };
        assert!(!status.is_clean());
    }

    #[test]
    fn test_file_entry_creation() {
        let entry = FileEntry::new("test.txt".to_string(), FileStatus::Added);
        assert_eq!(entry.path, "test.txt");
        assert_eq!(entry.status, FileStatus::Added);
    }

    #[test]
    fn test_file_status_types() {
        let added = FileEntry::new("added.txt".to_string(), FileStatus::Added);
        let modified = FileEntry::new("modified.txt".to_string(), FileStatus::Modified);
        let deleted = FileEntry::new("deleted.txt".to_string(), FileStatus::Deleted);
        let renamed = FileEntry::new("renamed.txt".to_string(), FileStatus::Renamed);
        let typechange = FileEntry::new("typechange.txt".to_string(), FileStatus::Typechange);

        assert_eq!(added.status, FileStatus::Added);
        assert_eq!(modified.status, FileStatus::Modified);
        assert_eq!(deleted.status, FileStatus::Deleted);
        assert_eq!(renamed.status, FileStatus::Renamed);
        assert_eq!(typechange.status, FileStatus::Typechange);
    }

    #[test]
    fn test_file_status_display_formats() {
        assert_eq!(FileStatus::Added.to_string(), "added");
        assert_eq!(FileStatus::Modified.to_string(), "modified");
        assert_eq!(FileStatus::Deleted.to_string(), "deleted");
        assert_eq!(FileStatus::Renamed.to_string(), "renamed");
        assert_eq!(FileStatus::Typechange.to_string(), "typechange");
    }

    // Test the directory expansion functionality
    #[test]
    fn test_expand_untracked_directories() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory structure for testing
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = temp_dir.path();

        // Create a subdirectory with some files
        let subdir = repo_path.join("new_dir");
        fs::create_dir(&subdir).expect("Failed to create subdirectory");

        let file1 = subdir.join("file1.txt");
        let file2 = subdir.join("file2.txt");
        fs::write(&file1, "content1").expect("Failed to write file1");
        fs::write(&file2, "content2").expect("Failed to write file2");

        // Create another nested directory with a file
        let nested_dir = subdir.join("nested");
        fs::create_dir(&nested_dir).expect("Failed to create nested directory");
        let nested_file = nested_dir.join("nested_file.txt");
        fs::write(&nested_file, "nested content").expect("Failed to write nested file");

        // Test the helper function directly
        let untracked_paths = vec!["new_dir".to_string()];
        let expanded = super::expand_untracked_directories(untracked_paths, repo_path);

        // Should expand to all individual files
        assert!(expanded.contains(&"new_dir/file1.txt".to_string()));
        assert!(expanded.contains(&"new_dir/file2.txt".to_string()));
        assert!(expanded.contains(&"new_dir/nested/nested_file.txt".to_string()));
        assert_eq!(expanded.len(), 3); // Three files total

        // Test with mixed files and directories
        let untracked_paths = vec!["new_dir".to_string(), "standalone.txt".to_string()];
        fs::write(repo_path.join("standalone.txt"), "standalone")
            .expect("Failed to write standalone file");
        let expanded = super::expand_untracked_directories(untracked_paths, repo_path);

        assert!(expanded.contains(&"new_dir/file1.txt".to_string()));
        assert!(expanded.contains(&"new_dir/file2.txt".to_string()));
        assert!(expanded.contains(&"new_dir/nested/nested_file.txt".to_string()));
        assert!(expanded.contains(&"standalone.txt".to_string()));
        assert_eq!(expanded.len(), 4); // Three files from directory + one standalone file
    }
}
