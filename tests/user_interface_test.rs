use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mahgit::{status::RepositoryStatus, ui::App};
use std::env;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

mod common;
mod test_backend_utils;

use common::create_test_repository;
use test_backend_utils::*;

fn create_key_event(ch: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(ch),
        modifiers: KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    }
}

#[test]
fn test_help_window() {
    let repository = create_test_repository()
        .expect("Failed to create test repository")
        .repository;
    let status = RepositoryStatus::empty();
    let mut app = App::new(repository, status);

    let help_key = create_key_event('?');

    // Initially help should not be showing
    assert!(!app.is_showing_help(), "Help should be hidden initially");

    // Toggle help on
    app.process_key_event(help_key);
    assert!(app.is_showing_help(), "Help should show after pressing '?'");

    // Toggle help off
    app.process_key_event(help_key);
    assert!(
        !app.is_showing_help(),
        "Help should hide after pressing '?' again"
    );
}

#[test]
fn test_refresh_functionality() {
    let test_repo = create_test_repository()
        .expect("Failed to create test repository")
        .temp_dir;
    let repository = mahgit::repository::Repository::discover(test_repo.path()).unwrap();

    std::env::set_current_dir(test_repo.path()).unwrap();

    let mut test_app = TestApp::with_repository(80, 24, repository).unwrap();
    test_app.render().unwrap();

    // Verify the absence of a test file
    let test_filename = "test_new_file.txt";
    assert!(
        !test_app.assert_contains(test_filename),
        "Test file should not exist initially"
    );

    // Add a new file to the repository
    std::fs::write(test_repo.path().join(test_filename), "test content").unwrap();

    // Trigger refresh
    test_app.refresh().unwrap();
    test_app.render().unwrap();

    // Verify the presence of the file name in the list
    assert!(
        test_app.assert_contains(test_filename),
        "Test file should be visible after refresh"
    );

    // Should still show branch info after refresh
    assert!(
        test_app.assert_contains("main"),
        "Branch should be visible after refresh"
    );
}

#[test]
fn test_quit_functionality() {
    let test_repo = create_test_repository()
        .expect("Failed to create test repository")
        .temp_dir;
    let repository = mahgit::repository::Repository::discover(test_repo.path()).unwrap();

    std::env::set_current_dir(test_repo.path()).unwrap();

    let mut test_app = TestApp::with_repository(80, 24, repository).unwrap();
    test_app.render().unwrap();

    // Test quit
    test_app.quit().unwrap();
    test_app.render().unwrap();

    assert!(test_app.should_quit(), "App should quit after quit command");
}

/// Test console mode (non-interactive execution)
#[test]
fn test_console_mode() {
    let test_repo = create_test_repository().expect("Failed to create test repository");

    let original_dir = env::current_dir().unwrap();

    // Use a closure to ensure cleanup even if test fails
    let test_result = std::panic::catch_unwind(|| {
        env::set_current_dir(test_repo.temp_dir.path()).expect("Failed to change directory");

        // Test console mode (non-interactive)
        let mahgit_path = get_mahgit_binary_path();
        let output = Command::new(&mahgit_path)
            .arg("--console")
            .output()
            .expect("Failed to run mahgit in console mode");

        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).to_string(),
        )
    });

    // Always try to restore directory, but don't fail the test if restoration fails
    if original_dir.exists() {
        let _ = env::set_current_dir(&original_dir);
    }

    // Handle the test result
    match test_result {
        Ok((success, stdout)) => {
            assert!(success, "Console mode should succeed");
            assert!(stdout.contains("main"), "Should show branch name");
        }
        Err(e) => {
            std::panic::resume_unwind(e);
        }
    }

    // Keep test_repo in scope until the end to prevent temp directory cleanup
    drop(test_repo);
}

/// Test error handling when running outside a git repository
#[test]
fn test_error_handling_outside_repo() {
    // Test starting mahgit outside a git repository
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_dir.path()).expect("Failed to change directory");

    let mahgit_path = get_mahgit_binary_path();
    let output = Command::new(&mahgit_path)
        .arg("--console")
        .output()
        .expect("Failed to run mahgit");

    // Should fail with appropriate exit code
    assert!(!output.status.success(), "Should fail outside git repo");

    env::set_current_dir(original_dir).expect("Failed to restore directory");
}

fn get_mahgit_binary_path() -> PathBuf {
    // Look for the binary in the target directory relative to the current working directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(manifest_dir)
        .join("target")
        .join("debug")
        .join("mahgit")
}
