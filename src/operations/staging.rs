use super::OperationResult;
use crate::repository::{Repository, RepositoryError};

pub struct StagingOperations<'repo> {
    repository: &'repo Repository,
}

impl<'repo> StagingOperations<'repo> {
    pub fn new(repository: &'repo Repository) -> Self {
        Self { repository }
    }

    pub fn stage_file(&self, path: &str) -> Result<OperationResult, RepositoryError> {
        self.repository.add_to_index(path)?;
        Ok(OperationResult::new(format!("Staged {}", path)))
    }

    pub fn unstage_file(&self, path: &str) -> Result<OperationResult, RepositoryError> {
        self.repository.reset_file(path)?;
        Ok(OperationResult::new(format!("Unstaged {}", path)))
    }

    pub fn add_untracked_file(&self, path: &str) -> Result<OperationResult, RepositoryError> {
        self.repository.add_to_index(path)?;
        Ok(OperationResult::new(format!("Added {}", path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    struct TestRepo {
        _temp_dir: TempDir,
        repo: Repository,
    }

    impl TestRepo {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let temp_dir = TempDir::new()?;
            let repo_path = temp_dir.path();

            // Initialize git repository
            let git_repo = git2::Repository::init(repo_path)?;

            // Create initial commit to avoid empty repository issues
            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = {
                let mut index = git_repo.index()?;
                index.write_tree()?
            };
            let tree = git_repo.find_tree(tree_id)?;

            git_repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])?;

            let repo = Repository::open(repo_path)?;

            Ok(TestRepo {
                _temp_dir: temp_dir,
                repo,
            })
        }

        fn create_file(&self, path: &str, content: &str) -> Result<(), Box<dyn std::error::Error>> {
            let file_path = self._temp_dir.path().join(path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(file_path, content)?;
            Ok(())
        }
    }

    #[test]
    fn test_add_untracked_file() {
        let test_repo = TestRepo::new().expect("Failed to create test repository");
        test_repo
            .create_file("test.txt", "content")
            .expect("Failed to create test file");

        let ops = StagingOperations::new(&test_repo.repo);
        let result = ops
            .add_untracked_file("test.txt")
            .expect("Operation failed");

        assert!(result.message.contains("Added test.txt"));
    }

    #[test]
    fn test_stage_nonexistent_file() {
        let test_repo = TestRepo::new().expect("Failed to create test repository");
        let ops = StagingOperations::new(&test_repo.repo);

        let result = ops.stage_file("nonexistent.txt");
        assert!(result.is_err());
    }
}
