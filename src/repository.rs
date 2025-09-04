use git2::Repository as Git2Repository;
use std::path::Path;

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
        self.git_repo
            .statuses(None)
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
        index
            .add_path(std::path::Path::new(path))
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;
        index
            .write()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;
        Ok(())
    }

    pub fn reset_file(&self, path: &str) -> Result<(), RepositoryError> {
        let head_commit = self
            .git_repo
            .head()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?
            .peel_to_commit()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        let head_tree = head_commit
            .tree()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        let head_object = head_tree.as_object();

        self.git_repo
            .reset_default(Some(head_object), [path])
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;

        Ok(())
    }

    pub fn git2_repo(&self) -> &Git2Repository {
        &self.git_repo
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

        let io_error = std::io::Error::new(std::io::ErrorKind::Other, "test io error");
        let repo_error = RepositoryError::IoError(io_error);
        assert!(repo_error.to_string().contains("IO error"));
    }
}
