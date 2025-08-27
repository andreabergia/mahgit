use crate::repository::{Repository, RepositoryError};
use git2::Status;

#[derive(Debug, Default)]
pub struct RepositoryStatus {
    pub branch_name: String,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
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
                if status.intersects(
                    Status::INDEX_NEW
                        | Status::INDEX_MODIFIED
                        | Status::INDEX_DELETED
                        | Status::INDEX_RENAMED
                        | Status::INDEX_TYPECHANGE,
                ) {
                    staged.push(path.clone());
                }
                if status.intersects(
                    Status::WT_MODIFIED
                        | Status::WT_DELETED
                        | Status::WT_RENAMED
                        | Status::WT_TYPECHANGE,
                ) {
                    unstaged.push(path);
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

    pub fn staged_files(&self) -> &Vec<String> {
        &self.staged
    }

    pub fn unstaged_files(&self) -> &Vec<String> {
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
            staged: vec!["file.txt".to_string()],
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
            unstaged: vec!["file.txt".to_string()],
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
}
