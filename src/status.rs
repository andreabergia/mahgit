use crate::repository::{Repository, RepositoryError};

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
        
        Ok(RepositoryStatus {
            branch_name,
            staged: Vec::new(),
            unstaged: Vec::new(),
            untracked: Vec::new(),
            conflicted: Vec::new(),
        })
    }

    pub fn is_clean(&self) -> bool {
        self.staged.is_empty() 
            && self.unstaged.is_empty() 
            && self.untracked.is_empty() 
            && self.conflicted.is_empty()
    }
}