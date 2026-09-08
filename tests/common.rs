use mahgit::repository::Repository;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use tempfile::TempDir;

static CURRENT_DIR_LOCK: Mutex<()> = Mutex::new(());

#[allow(dead_code)]
pub struct CurrentDirGuard {
    original_dir: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        env::set_current_dir(&self.original_dir).expect("Failed to restore current directory");
    }
}

#[allow(dead_code)]
pub fn change_current_dir(path: &Path) -> CurrentDirGuard {
    let lock = CURRENT_DIR_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let original_dir = env::current_dir().expect("Failed to read current working directory");
    env::set_current_dir(path).expect("Failed to change current directory");
    CurrentDirGuard {
        original_dir,
        _lock: lock,
    }
}

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
        .args(["init", "--initial-branch=main"])
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
    fs::write(temp_dir.path().join(filename), content).expect("Failed to create test file");
}
