use mahgit::repository::{Repository, RepositoryError};
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

        // Configure user for commits
        let mut config = git_repo.config()?;
        config.set_str("user.name", "Test User")?;
        config.set_str("user.email", "test@example.com")?;

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

    fn create_and_commit_file(
        &self,
        path: &str,
        content: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let file_path = self._temp_dir.path().join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(file_path, content)?;

        // Add to index
        self.repo.add_to_index(path)?;

        // Create commit
        let git_repo = self.repo.git2_repo();
        let mut index = git_repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = git_repo.find_tree(tree_id)?;

        let head = git_repo.head()?;
        let parent = git_repo.find_commit(head.target().unwrap())?;

        let sig = git2::Signature::now("Test", "test@example.com")?;
        git_repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &format!("Add {}", path),
            &tree,
            &[&parent],
        )?;

        Ok(())
    }

    fn delete_file(&self, path: &str) -> Result<(), std::io::Error> {
        let file_path = self._temp_dir.path().join(path);
        fs::remove_file(file_path)
    }

    fn delete_directory(&self, path: &str) -> Result<(), std::io::Error> {
        let dir_path = self._temp_dir.path().join(path);
        fs::remove_dir_all(dir_path)
    }
}

#[test]
fn test_stage_deleted_file() {
    let test_repo = TestRepo::new().expect("Failed to create test repository");

    // Create and commit a file
    test_repo
        .create_and_commit_file("test.txt", "content")
        .expect("Failed to create and commit file");

    // Verify the file exists
    assert!(test_repo._temp_dir.path().join("test.txt").exists());

    // Delete the file from the filesystem
    test_repo
        .delete_file("test.txt")
        .expect("Failed to delete file");

    // Verify the file no longer exists in the filesystem
    assert!(!test_repo._temp_dir.path().join("test.txt").exists());

    // Now try to stage the deleted file - this should work without error
    // The file should be in the index (since it was committed) but not in the working directory
    let result = test_repo.repo.add_to_index("test.txt");

    // The operation should succeed (no error)
    assert!(
        result.is_ok(),
        "Staging deleted file should succeed, but got error: {:?}",
        result.err()
    );

    // Verify deletion is actually staged in index
    let statuses = test_repo
        .repo
        .get_statuses()
        .expect("Failed to read statuses");
    let status = statuses
        .iter()
        .find(|entry| entry.path() == Some("test.txt"))
        .expect("Expected status entry for test.txt")
        .status();

    assert!(status.contains(git2::Status::INDEX_DELETED));
    assert!(!status.contains(git2::Status::WT_DELETED));
}

#[test]
fn test_stage_nonexistent_and_untracked_file() {
    let test_repo = TestRepo::new().expect("Failed to create test repository");

    // Try to stage a file that never existed and is not in the index
    let result = test_repo.repo.add_to_index("never_existed.txt");

    // This should fail with the appropriate error
    assert!(result.is_err());

    // Check that it's the expected error
    match result.err().unwrap() {
        RepositoryError::Other(msg) => {
            assert!(msg.contains("does not exist and is not tracked"));
        }
        _ => panic!("Expected RepositoryError::Other with 'does not exist' message"),
    }
}

#[test]
fn test_stage_deleted_file_with_dot_slash_path() {
    let test_repo = TestRepo::new().expect("Failed to create test repository");
    test_repo
        .create_and_commit_file("nested/test.txt", "content")
        .expect("Failed to create and commit file");
    test_repo
        .delete_file("nested/test.txt")
        .expect("Failed to delete file");

    let result = test_repo.repo.add_to_index("./nested/test.txt");
    assert!(
        result.is_ok(),
        "Staging deleted file via ./ path should succeed, got: {:?}",
        result.err()
    );
}

#[test]
fn test_stage_deleted_tracked_directory_stages_nested_deletions() {
    let test_repo = TestRepo::new().expect("Failed to create test repository");
    test_repo
        .create_and_commit_file("dir/a.txt", "content-a")
        .expect("Failed to create and commit dir/a.txt");
    test_repo
        .create_and_commit_file("dir/sub/b.txt", "content-b")
        .expect("Failed to create and commit dir/sub/b.txt");
    test_repo
        .delete_directory("dir")
        .expect("Failed to delete tracked directory");

    let result = test_repo.repo.add_to_index("dir");
    assert!(
        result.is_ok(),
        "Staging deleted tracked directory should succeed, got: {:?}",
        result.err()
    );

    let statuses = test_repo
        .repo
        .get_statuses()
        .expect("Failed to read statuses");

    for path in ["dir/a.txt", "dir/sub/b.txt"] {
        let status = statuses
            .iter()
            .find(|entry| entry.path() == Some(path))
            .unwrap_or_else(|| panic!("Expected status entry for {path}"))
            .status();

        assert!(
            status.contains(git2::Status::INDEX_DELETED),
            "Expected INDEX_DELETED for {path}, got {:?}",
            status
        );
        assert!(
            !status.contains(git2::Status::WT_DELETED),
            "Expected WT_DELETED to be cleared for {path}, got {:?}",
            status
        );
    }
}
