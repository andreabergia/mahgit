use git2::Repository as Git2Repository;
use std::path::Path;

#[derive(Debug)]
pub enum RepositoryError {
    NotFound,
    NotARepository,
    Corrupted(String),
    AccessDenied,
    Other(String),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryError::NotFound => write!(f, "Not in a Git repository"),
            RepositoryError::NotARepository => write!(f, "Not a Git repository"),
            RepositoryError::Corrupted(msg) => write!(f, "Repository corrupted: {}", msg),
            RepositoryError::AccessDenied => write!(f, "Access denied to repository"),
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
        let git_repo = Git2Repository::discover(path)
            .map_err(|e| match e.code() {
                git2::ErrorCode::NotFound => RepositoryError::NotFound,
                _ => RepositoryError::Other(e.message().to_string()),
            })?;

        Ok(Repository { git_repo })
    }

    pub fn current_branch_name(&self) -> Result<String, RepositoryError> {
        let head = self.git_repo.head()
            .map_err(|e| RepositoryError::Other(e.message().to_string()))?;
        
        if let Some(name) = head.shorthand() {
            Ok(name.to_string())
        } else {
            Ok("HEAD".to_string())
        }
    }
}