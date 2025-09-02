use mahgit::repository::Repository;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[allow(dead_code)]
pub struct TestRepository {
    pub temp_dir: TempDir,
    pub repository: Repository,
}

/// Creates a test Git repository with initial commit
/// Returns (TempDir, Repository) tuple for use in tests
#[allow(dead_code)]
pub fn create_test_repository() -> Result<TestRepository, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;

    // Initialize git repository
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()?;

    // Set git config
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_dir.path())
        .output()?;

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(temp_dir.path())
        .output()?;

    // Create initial commit
    std::fs::write(temp_dir.path().join("README.md"), "# Test Repository\n")?;
    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(temp_dir.path())
        .output()?;

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()?;

    let repository = Repository::open(temp_dir.path())?;
    Ok(TestRepository {
        temp_dir,
        repository,
    })
}

/// Helper function to create a test file in a temporary directory
#[allow(dead_code)]
pub fn create_test_file(temp_dir: &TempDir, filename: &str, content: &str) {
    fs::write(temp_dir.path().join(filename), content)
        .expect("Failed to create test file");
}
