use mahgit::repository::Repository;
use mahgit::status::RepositoryStatus;
use std::fs;
use tempfile::TempDir;

// Helper function to create an initial commit in the git repository
fn create_initial_commit(git_repo: &git2::Repository, repo_path: &std::path::Path) {
    // Create a dummy file to commit
    let dummy_file = repo_path.join("README.md");
    fs::write(&dummy_file, "# Test Repository\n").expect("Failed to write README");

    // Add the file to the index
    let mut index = git_repo.index().expect("Failed to get index");
    index
        .add_path(std::path::Path::new("README.md"))
        .expect("Failed to add file to index");
    index.write().expect("Failed to write index");

    // Get the tree from the index
    let tree_id = index.write_tree().expect("Failed to write tree");
    let tree = git_repo.find_tree(tree_id).expect("Failed to find tree");

    // Create the initial commit
    let signature =
        git2::Signature::now("Test User", "test@example.com").expect("Failed to create signature");

    git_repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )
        .expect("Failed to create commit");
}

#[test]
fn test_untracked_directories_expanded_to_individual_files() {
    // Create a temporary directory for our test repository
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path();

    // Initialize a git repository using git2 directly
    let git_repo = git2::Repository::init(repo_path).expect("Failed to initialize git repo");

    // Create an initial commit to establish the main branch
    create_initial_commit(&git_repo, repo_path);

    // Open the repository using mahgit's Repository
    let repository = Repository::open(repo_path).expect("Failed to open repository");

    // Create a new directory with multiple files
    let new_dir = repo_path.join("new_directory");
    fs::create_dir(&new_dir).expect("Failed to create new directory");

    let file1 = new_dir.join("file1.txt");
    let file2 = new_dir.join("file2.txt");
    fs::write(&file1, "content1").expect("Failed to write file1");
    fs::write(&file2, "content2").expect("Failed to write file2");

    // Create a nested directory with a file
    let nested_dir = new_dir.join("nested");
    fs::create_dir(&nested_dir).expect("Failed to create nested directory");
    let nested_file = nested_dir.join("nested_file.rs");
    fs::write(&nested_file, "// Rust code").expect("Failed to write nested file");

    // Get the repository status
    let status = RepositoryStatus::new(&repository).expect("Failed to get repository status");

    // Verify that the untracked files list contains individual files, not just the directory
    let untracked = status.untracked_files();

    // Check that we have the individual files, not just the directory name
    assert!(untracked.contains(&"new_directory/file1.txt".to_string()));
    assert!(untracked.contains(&"new_directory/file2.txt".to_string()));
    assert!(untracked.contains(&"new_directory/nested/nested_file.rs".to_string()));

    // Should not contain just the directory name
    assert!(!untracked.contains(&"new_directory".to_string()));

    // Total should be 3 files
    assert_eq!(untracked.len(), 3);
}

#[test]
fn test_mixed_untracked_files_and_directories() {
    // Create a temporary directory for our test repository
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path();

    // Initialize a git repository using git2 directly
    let git_repo = git2::Repository::init(repo_path).expect("Failed to initialize git repo");

    // Create an initial commit to establish the main branch
    create_initial_commit(&git_repo, repo_path);

    // Open the repository using mahgit's Repository
    let repository = Repository::open(repo_path).expect("Failed to open repository");

    // Create a new directory with files
    let new_dir = repo_path.join("my_new_dir");
    fs::create_dir(&new_dir).expect("Failed to create new directory");
    let dir_file = new_dir.join("dir_file.txt");
    fs::write(&dir_file, "dir content").expect("Failed to write directory file");

    // Create a standalone file
    let standalone_file = repo_path.join("standalone.txt");
    fs::write(&standalone_file, "standalone content").expect("Failed to write standalone file");

    // Get the repository status
    let status = RepositoryStatus::new(&repository).expect("Failed to get repository status");

    // Verify that both the individual file from the directory and the standalone file are listed
    let untracked = status.untracked_files();

    assert!(untracked.contains(&"my_new_dir/dir_file.txt".to_string()));
    assert!(untracked.contains(&"standalone.txt".to_string()));

    // Should not contain just the directory name
    assert!(!untracked.contains(&"my_new_dir".to_string()));

    // Total should be 2 files
    assert_eq!(untracked.len(), 2);
}
