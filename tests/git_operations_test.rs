use mahgit::repository::{Repository, RepositoryError};
use mahgit::status::{FileStatus, RepositoryStatus};
use std::fs;
use tempfile::TempDir;

fn create_test_repo_with_changes() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let repo = git2::Repository::init(&temp)?;

    let sig = git2::Signature::new("Test User", "test@example.com", &git2::Time::new(0, 0))?;

    fs::write(temp.path().join("file1.txt"), "initial content")?;

    let mut index = repo.index()?;
    index.add_path(std::path::Path::new("file1.txt"))?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])?;

    Ok(temp)
}

#[test]
fn test_repository_discovery_in_valid_repo() {
    let temp_repo = create_test_repo_with_changes().unwrap();

    let result = Repository::discover(temp_repo.path());
    assert!(result.is_ok());
}

#[test]
fn test_repository_discovery_outside_repo() {
    let temp = TempDir::new().unwrap();

    let result = Repository::discover(temp.path());
    assert!(result.is_err());

    assert!(matches!(result, Err(RepositoryError::NotFound)));
}

#[test]
fn test_clean_repository_status() {
    let temp_repo = create_test_repo_with_changes().unwrap();
    let repo = Repository::discover(temp_repo.path()).unwrap();

    let status = RepositoryStatus::new(&repo).unwrap();
    assert!(status.is_clean());
    assert_eq!(status.branch_name, "main");
}

#[test]
fn test_untracked_files_status() {
    let temp_repo = create_test_repo_with_changes().unwrap();

    fs::write(temp_repo.path().join("untracked.txt"), "untracked content").unwrap();

    let repo = Repository::discover(temp_repo.path()).unwrap();
    let status = RepositoryStatus::new(&repo).unwrap();

    assert!(!status.is_clean());
    assert_eq!(status.untracked.len(), 1);
    assert!(status.untracked.contains(&"untracked.txt".to_string()));
    assert!(status.staged.is_empty());
    assert!(status.unstaged.is_empty());
    assert!(status.conflicted.is_empty());
}

#[test]
fn test_unstaged_changes_status() {
    let temp_repo = create_test_repo_with_changes().unwrap();

    fs::write(temp_repo.path().join("file1.txt"), "modified content").unwrap();

    let repo = Repository::discover(temp_repo.path()).unwrap();
    let status = RepositoryStatus::new(&repo).unwrap();

    assert!(!status.is_clean());
    assert_eq!(status.unstaged.len(), 1);
    assert!(
        status
            .unstaged
            .iter()
            .any(|entry| entry.path == "file1.txt" && entry.status == FileStatus::Modified)
    );
    assert!(status.staged.is_empty());
    assert!(status.untracked.is_empty());
    assert!(status.conflicted.is_empty());
}

#[test]
fn test_staged_changes_status() {
    let temp_repo = create_test_repo_with_changes().unwrap();
    let repo_git2 = git2::Repository::discover(temp_repo.path()).unwrap();

    fs::write(temp_repo.path().join("file2.txt"), "new staged content").unwrap();

    let mut index = repo_git2.index().unwrap();
    index.add_path(std::path::Path::new("file2.txt")).unwrap();
    index.write().unwrap();

    let repo = Repository::discover(temp_repo.path()).unwrap();
    let status = RepositoryStatus::new(&repo).unwrap();

    assert!(!status.is_clean());
    assert_eq!(status.staged.len(), 1);
    assert!(
        status
            .staged
            .iter()
            .any(|entry| entry.path == "file2.txt" && entry.status == FileStatus::Added)
    );
    assert!(status.unstaged.is_empty());
    assert!(status.untracked.is_empty());
    assert!(status.conflicted.is_empty());
}

#[test]
fn test_branch_name_detection() {
    let temp_repo = create_test_repo_with_changes().unwrap();
    let repo = Repository::discover(temp_repo.path()).unwrap();

    let branch_name = repo.current_branch_name().unwrap();
    assert_eq!(branch_name, "main");
}

#[test]
fn test_repository_status_reload() {
    let temp_repo = create_test_repo_with_changes().unwrap();
    let repo = Repository::discover(temp_repo.path()).unwrap();

    // Initial status should be clean
    let mut status = RepositoryStatus::new(&repo).unwrap();
    assert!(status.is_clean());
    assert_eq!(status.untracked.len(), 0);

    // Add an untracked file to the repository
    fs::write(temp_repo.path().join("new_file.txt"), "new content").unwrap();

    // Status should still show as clean because we haven't reloaded
    assert!(status.is_clean());
    assert_eq!(status.untracked.len(), 0);

    // After reload, status should reflect the new untracked file
    status.reload(&repo).unwrap();
    assert!(!status.is_clean());
    assert_eq!(status.untracked.len(), 1);
    assert!(status.untracked.contains(&"new_file.txt".to_string()));

    // Stage the file using git2
    let repo_git2 = git2::Repository::discover(temp_repo.path()).unwrap();
    let mut index = repo_git2.index().unwrap();
    index
        .add_path(std::path::Path::new("new_file.txt"))
        .unwrap();
    index.write().unwrap();

    // Reload again and verify the file is now staged instead of untracked
    status.reload(&repo).unwrap();
    assert!(!status.is_clean());
    assert_eq!(status.untracked.len(), 0);
    assert_eq!(status.staged.len(), 1);
    assert!(
        status
            .staged
            .iter()
            .any(|entry| entry.path == "new_file.txt" && entry.status == FileStatus::Added)
    );
}
