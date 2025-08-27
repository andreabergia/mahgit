use crate::repository::{Repository, RepositoryError};
use git2::Status;
use std::fmt;

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
}

impl FileEntry {
    pub fn new(path: String, status: FileStatus) -> Self {
        Self { path, status }
    }
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

        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("<invalid utf-8>").to_string();
            let status = entry.status();

            if status.contains(Status::CONFLICTED) {
                conflicted.push(path);
            } else if status.contains(Status::WT_NEW) {
                untracked.push(path);
            } else {
                // Handle staged changes
                if status.contains(Status::INDEX_NEW) {
                    staged.push(FileEntry::new(path.clone(), FileStatus::Added));
                } else if status.contains(Status::INDEX_MODIFIED) {
                    staged.push(FileEntry::new(path.clone(), FileStatus::Modified));
                } else if status.contains(Status::INDEX_DELETED) {
                    staged.push(FileEntry::new(path.clone(), FileStatus::Deleted));
                } else if status.contains(Status::INDEX_RENAMED) {
                    staged.push(FileEntry::new(path.clone(), FileStatus::Renamed));
                } else if status.contains(Status::INDEX_TYPECHANGE) {
                    staged.push(FileEntry::new(path.clone(), FileStatus::Typechange));
                }

                // Handle unstaged changes
                if status.contains(Status::WT_MODIFIED) {
                    unstaged.push(FileEntry::new(path.clone(), FileStatus::Modified));
                } else if status.contains(Status::WT_DELETED) {
                    unstaged.push(FileEntry::new(path.clone(), FileStatus::Deleted));
                } else if status.contains(Status::WT_RENAMED) {
                    unstaged.push(FileEntry::new(path.clone(), FileStatus::Renamed));
                } else if status.contains(Status::WT_TYPECHANGE) {
                    unstaged.push(FileEntry::new(path.clone(), FileStatus::Typechange));
                }
            }
        }

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

    // Note: The reload() method is tested in integration tests where we can
    // create actual git repositories and verify that reload() properly refreshes
    // the status from the actual repository state. Unit testing reload() would
    // just duplicate the implementation logic without adding value.
}
