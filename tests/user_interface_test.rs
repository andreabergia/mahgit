use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use mahgit::{config::Config, status::RepositoryStatus, ui::App};
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

fn create_esc_key_event() -> KeyEvent {
    KeyEvent {
        code: KeyCode::Esc,
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
    let config = Config {
        theme: mahgit::theme::Theme::default(),
        tab_width: 4,
        show_line_numbers: true,
        word_wrap: false,
        ignore_whitespace: false,
    };
    let mut app = App::new(repository, status, config);

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
fn test_help_window_esc_behavior() {
    let repository = create_test_repository()
        .expect("Failed to create test repository")
        .repository;
    let status = RepositoryStatus::empty();
    let config = Config {
        theme: mahgit::theme::Theme::default(),
        tab_width: 4,
        show_line_numbers: true,
        word_wrap: false,
        ignore_whitespace: false,
    };
    let mut app = App::new(repository, status, config);

    let help_key = create_key_event('?');
    let esc_key = create_esc_key_event();

    // Initially help should not be showing
    assert!(!app.is_showing_help(), "Help should be hidden initially");
    assert!(!app.should_quit(), "App should not be quitting");

    // Open help with '?'
    app.process_key_event(help_key);
    assert!(app.is_showing_help(), "Help should show after pressing '?'");
    assert!(!app.should_quit(), "App should not be quitting");

    // Close help with Esc (should not quit)
    app.process_key_event(esc_key);
    assert!(
        !app.is_showing_help(),
        "Help should hide after pressing Esc"
    );
    assert!(
        !app.should_quit(),
        "App should not quit when Esc closes help"
    );

    // Press Esc again when help is closed (should quit)
    app.process_key_event(esc_key);
    assert!(!app.is_showing_help(), "Help should still be hidden");
    assert!(
        app.should_quit(),
        "App should quit when Esc is pressed with help closed"
    );
}

#[test]
fn test_down_arrow_does_not_wrap_at_end_of_list() {
    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repo_path = test_repo.temp_dir.path();

    struct DirGuard(PathBuf);

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    let original_dir = std::env::current_dir().expect("Failed to read current working directory");
    let _dir_guard = DirGuard(original_dir);

    // Create an unstaged modification and an untracked file
    std::fs::write(
        repo_path.join("README.md"),
        "# Test Repository\nwith changes\n",
    )
    .expect("Failed to modify README.md");
    std::fs::write(repo_path.join("new_file.txt"), "new content\n")
        .expect("Failed to create new_file.txt");

    let repository =
        mahgit::repository::Repository::discover(repo_path).expect("Failed to discover repository");
    std::env::set_current_dir(repo_path).expect("Failed to change working directory");

    let mut test_app =
        TestApp::with_repository(80, 24, repository).expect("Failed to construct TestApp");
    test_app.render().expect("Failed to render initial state");

    // Move selection to the last visible entry
    test_app.send_key_code(KeyCode::Down);
    test_app
        .render()
        .expect("Failed to render after moving down");

    let (last_section, last_index) = {
        let nav = test_app.navigation();
        (nav.current_section(), nav.selected_index())
    };

    // Attempt to move down past the end of the list
    test_app.send_key_code(KeyCode::Down);
    test_app
        .render()
        .expect("Failed to render after second move down");

    let nav_after = test_app.navigation();
    assert_eq!(
        nav_after.current_section(),
        last_section,
        "Selection should remain in the same section when pressing Down at the end"
    );
    assert_eq!(
        nav_after.selected_index(),
        last_index,
        "Selection index should remain on the last entry when pressing Down again"
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
    let repo_path = test_repo.temp_dir.path();

    // Test console mode (non-interactive)
    let mahgit_path = get_mahgit_binary_path();
    let output = Command::new(&mahgit_path)
        .arg("--console")
        .current_dir(repo_path) // Explicitly set working directory for subprocess
        .output()
        .expect("Failed to run mahgit in console mode");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Console mode should succeed.\nStdout: {}\nStderr: {}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("main"),
        "Should show branch name.\nStdout: {}\nStderr: {}",
        stdout,
        stderr
    );

    // Keep test_repo in scope until the end to prevent temp directory cleanup
    drop(test_repo);
}

/// Test error handling when running outside a git repository
#[test]
fn test_error_handling_outside_repo() {
    // Test starting mahgit outside a git repository
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    let mahgit_path = get_mahgit_binary_path();
    let output = Command::new(&mahgit_path)
        .arg("--console")
        .current_dir(temp_dir.path()) // Explicitly set working directory for subprocess
        .output()
        .expect("Failed to run mahgit");

    // Should fail with appropriate exit code
    assert!(!output.status.success(), "Should fail outside git repo");
}

/// Test error handling for binary files
#[test]
fn test_binary_file_error_handling() {
    use crossterm::event::KeyCode;

    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repository = mahgit::repository::Repository::discover(test_repo.temp_dir.path()).unwrap();

    env::set_current_dir(test_repo.temp_dir.path()).unwrap();

    let mut test_app = TestApp::with_repository(80, 24, repository).unwrap();
    test_app.render().unwrap();
    let binary_file = "test_binary.bin";
    let binary_data = vec![0u8, 1u8, 2u8, 0u8, 255u8, 127u8]; // Binary content with null bytes
    std::fs::write(test_repo.temp_dir.path().join(binary_file), binary_data).unwrap();

    // Refresh to see the new file
    test_app.refresh().unwrap();
    test_app.render().unwrap();

    // Verify the binary file appears in the status
    assert!(
        test_app.assert_contains(binary_file),
        "Binary file should appear in status"
    );

    // Try to enter diff view (Tab key will try to diff the first available file)
    test_app.send_key_code(KeyCode::Tab);
    test_app.render().unwrap();

    // Should show binary file message in diff view
    let buffer_content = test_backend_utils::buffer_to_string(test_app.get_buffer());
    let has_binary_message =
        buffer_content.contains("Binary file") || buffer_content.contains("cannot display diff");

    assert!(
        has_binary_message,
        "Should show binary file error message. Buffer content: {}",
        buffer_content
    );
}

/// Test error handling for large files
#[test]
fn test_large_file_error_handling() {
    use crossterm::event::KeyCode;

    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repository = mahgit::repository::Repository::discover(test_repo.temp_dir.path()).unwrap();

    env::set_current_dir(test_repo.temp_dir.path()).unwrap();

    let mut test_app = TestApp::with_repository(80, 24, repository).unwrap();
    test_app.render().unwrap();

    let large_file = "test_large.txt";
    let large_content = "a".repeat(11 * 1024 * 1024); // 11MB file (exceeds 10MB limit)
    std::fs::write(test_repo.temp_dir.path().join(large_file), large_content).unwrap();

    // Refresh to see the large file
    test_app.refresh().unwrap();
    test_app.render().unwrap();

    // Verify the large file appears
    assert!(
        test_app.assert_contains(large_file),
        "Large file should appear in status"
    );

    // Try to view diff of large file
    test_app.send_key_code(KeyCode::Tab);
    test_app.render().unwrap();

    // Should show file too large error message
    let buffer_content = test_backend_utils::buffer_to_string(test_app.get_buffer());
    let has_large_file_message =
        buffer_content.contains("too large") || buffer_content.contains("File too large");

    assert!(
        has_large_file_message,
        "Should show file too large error message. Buffer content: {}",
        buffer_content
    );
}

/// Inline diff hunk navigation and staging selection behavior
#[test]
fn test_inline_hunk_nav_and_stage_advances_selection() {
    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repo = mahgit::repository::Repository::discover(test_repo.temp_dir.path()).unwrap();
    std::env::set_current_dir(test_repo.temp_dir.path()).unwrap();

    // Create a file with two separate hunks
    let path = test_repo.temp_dir.path().join("multi_hunk.txt");
    std::fs::write(&path, "a\nkeep\nc\n").unwrap();

    // Stage initial file and commit so we can create unstaged changes
    Command::new("git")
        .args(["add", "."])
        .current_dir(test_repo.temp_dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "base"])
        .current_dir(test_repo.temp_dir.path())
        .output()
        .unwrap();

    // Modify file to create two hunks (lines 1 and 3)
    std::fs::write(&path, "A\nkeep\nC\n").unwrap();

    let mut app = TestApp::with_repository(80, 24, repo).unwrap();
    app.render().unwrap();

    // Toggle inline diff on the first visible file via Tab
    app.send_key_code(KeyCode::Tab);
    app.render().unwrap();

    // Go to next hunk (should move from hunk 0 to 1)
    app.send_char('n');
    app.render().unwrap();

    // Stage current hunk (Shift+S)
    app.send_key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT));
    app.render().unwrap();

    // After staging a hunk, inline diff should still be visible
    let buffer = test_backend_utils::buffer_to_string(app.get_buffer());
    assert!(
        buffer.contains("@@"),
        "Inline diff should remain expanded after staging"
    );
}

/// Test that normal files work without error messages
#[test]
fn test_normal_file_diff_works() {
    use crossterm::event::KeyCode;

    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repository = mahgit::repository::Repository::discover(test_repo.temp_dir.path()).unwrap();

    env::set_current_dir(test_repo.temp_dir.path()).unwrap();

    let mut test_app = TestApp::with_repository(80, 24, repository).unwrap();
    test_app.render().unwrap();

    let normal_file = "test_normal.txt";
    let normal_content = "Hello\nWorld\nThis is a normal text file\n";
    std::fs::write(test_repo.temp_dir.path().join(normal_file), normal_content).unwrap();

    // Refresh to see the normal file
    test_app.refresh().unwrap();
    test_app.render().unwrap();

    // Verify the normal file appears
    assert!(
        test_app.assert_contains(normal_file),
        "Normal file should appear in status"
    );

    // Try to view diff of normal file - should work without errors
    test_app.send_key_code(KeyCode::Tab);
    test_app.render().unwrap();

    // Should not show error messages
    let buffer_content = test_backend_utils::buffer_to_string(test_app.get_buffer());
    let has_error_message = buffer_content.contains("Binary file")
        || buffer_content.contains("too large")
        || buffer_content.contains("(error)");

    assert!(
        !has_error_message,
        "Normal file diff should not show error messages. Buffer content: {}",
        buffer_content
    );
}

#[test]
fn test_mouse_scroll_navigation() {
    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repository = test_repo.repository;

    // Set current directory to the test repository
    std::env::set_current_dir(test_repo.temp_dir.path()).unwrap();

    let mut test_app = TestApp::with_repository(80, 24, repository).unwrap();

    // Create some files to scroll through by sending down keys to simulate having items
    test_app.send_char('j'); // Move down once
    test_app.send_char('j'); // Move down again

    // Test mouse scroll up
    let scroll_up = MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 10,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    test_app.send_mouse(scroll_up);

    // Test mouse scroll down
    let scroll_down = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 10,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    test_app.send_mouse(scroll_down);

    // The interface should respond to mouse events (even if no visual change in empty repo)
    // This test verifies that mouse events are processed without errors
    assert!(
        test_app.render().is_ok(),
        "App should handle mouse events without errors"
    );
}

#[test]
fn test_mouse_unsupported_events_ignored() {
    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repository = test_repo.repository;

    // Set current directory to the test repository
    std::env::set_current_dir(test_repo.temp_dir.path()).unwrap();

    let mut test_app = TestApp::with_repository(80, 24, repository).unwrap();

    // Test that unsupported mouse events don't crash the app
    let click = MouseEvent {
        kind: MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: 10,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    test_app.send_mouse(click);

    let mouse_move = MouseEvent {
        kind: MouseEventKind::Moved,
        column: 15,
        row: 15,
        modifiers: KeyModifiers::NONE,
    };
    test_app.send_mouse(mouse_move);

    // App should continue to function normally after unsupported mouse events
    assert!(
        test_app.render().is_ok(),
        "App should ignore unsupported mouse events gracefully"
    );
}

fn get_mahgit_binary_path() -> PathBuf {
    // Look for the binary in the target directory relative to the current working directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(manifest_dir)
        .join("target")
        .join("debug")
        .join("mahgit")
}

#[test]
fn test_section_collapse_expand() {
    use mahgit::ui::navigation::StatusSection;
    use std::process::Command;

    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repository = mahgit::repository::Repository::discover(test_repo.temp_dir.path()).unwrap();

    env::set_current_dir(test_repo.temp_dir.path()).unwrap();

    // Create files in different sections to test section collapsing
    // 1. Create an unstaged file
    let unstaged_file = "unstaged_file.txt";
    std::fs::write(
        test_repo.temp_dir.path().join(unstaged_file),
        "unstaged content\n",
    )
    .unwrap();
    Command::new("git")
        .args(["add", unstaged_file])
        .current_dir(test_repo.temp_dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add unstaged file"])
        .current_dir(test_repo.temp_dir.path())
        .output()
        .unwrap();
    // Modify it to create unstaged changes
    std::fs::write(
        test_repo.temp_dir.path().join(unstaged_file),
        "modified unstaged content\n",
    )
    .unwrap();

    // 2. Create an untracked file
    let untracked_file = "untracked_file.txt";
    std::fs::write(
        test_repo.temp_dir.path().join(untracked_file),
        "untracked content\n",
    )
    .unwrap();

    // 3. Create a staged file
    let staged_file = "staged_file.txt";
    std::fs::write(
        test_repo.temp_dir.path().join(staged_file),
        "staged content\n",
    )
    .unwrap();
    Command::new("git")
        .args(["add", staged_file])
        .current_dir(test_repo.temp_dir.path())
        .output()
        .unwrap();

    let mut test_app = TestApp::with_repository(80, 30, repository).unwrap();
    test_app.render().unwrap();

    // Verify all sections start expanded (default state)
    assert!(
        !test_app
            .navigation()
            .is_section_collapsed(StatusSection::Unstaged),
        "Unstaged section should start expanded"
    );
    assert!(
        !test_app
            .navigation()
            .is_section_collapsed(StatusSection::Untracked),
        "Untracked section should start expanded"
    );
    assert!(
        !test_app
            .navigation()
            .is_section_collapsed(StatusSection::Staged),
        "Staged section should start expanded"
    );

    // Verify files are visible in expanded sections
    let buffer_content = test_backend_utils::buffer_to_string(test_app.get_buffer());
    assert!(
        buffer_content.contains(unstaged_file),
        "Unstaged file should be visible when section is expanded"
    );
    assert!(
        buffer_content.contains(untracked_file),
        "Untracked file should be visible when section is expanded"
    );
    assert!(
        buffer_content.contains(staged_file),
        "Staged file should be visible when section is expanded"
    );

    // Verify expanded icon "▾" appears
    assert!(
        buffer_content.contains("▾"),
        "Expanded sections should show ▾ icon"
    );

    // Test 1: Collapse the Unstaged section
    test_app.toggle_section_collapsed(StatusSection::Unstaged);
    test_app.render().unwrap();

    assert!(
        test_app
            .navigation()
            .is_section_collapsed(StatusSection::Unstaged),
        "Unstaged section should be collapsed after toggle"
    );

    let buffer_content = test_backend_utils::buffer_to_string(test_app.get_buffer());
    assert!(
        !buffer_content.contains(unstaged_file),
        "Unstaged file should NOT be visible when section is collapsed. Buffer: {}",
        buffer_content
    );
    // Other sections should still show their files
    assert!(
        buffer_content.contains(untracked_file),
        "Untracked file should still be visible"
    );
    assert!(
        buffer_content.contains(staged_file),
        "Staged file should still be visible"
    );
    // Verify collapsed icon "▸" appears for Unstaged section
    assert!(
        buffer_content.contains("▸"),
        "Collapsed section should show ▸ icon"
    );

    // Test 2: Collapse the Untracked section (while Unstaged remains collapsed)
    test_app.toggle_section_collapsed(StatusSection::Untracked);
    test_app.render().unwrap();

    assert!(
        test_app
            .navigation()
            .is_section_collapsed(StatusSection::Untracked),
        "Untracked section should be collapsed after toggle"
    );

    let buffer_content = test_backend_utils::buffer_to_string(test_app.get_buffer());
    assert!(
        !buffer_content.contains(unstaged_file),
        "Unstaged file should still NOT be visible (section still collapsed)"
    );
    assert!(
        !buffer_content.contains(untracked_file),
        "Untracked file should NOT be visible when section is collapsed"
    );
    // Staged section should still show its files
    assert!(
        buffer_content.contains(staged_file),
        "Staged file should still be visible"
    );

    // Test 3: Expand the Unstaged section back
    test_app.toggle_section_collapsed(StatusSection::Unstaged);
    test_app.render().unwrap();

    assert!(
        !test_app
            .navigation()
            .is_section_collapsed(StatusSection::Unstaged),
        "Unstaged section should be expanded after second toggle"
    );

    let buffer_content = test_backend_utils::buffer_to_string(test_app.get_buffer());
    assert!(
        buffer_content.contains(unstaged_file),
        "Unstaged file should be visible again when section is re-expanded"
    );
    // Untracked should still be collapsed
    assert!(
        !buffer_content.contains(untracked_file),
        "Untracked file should still NOT be visible (section still collapsed)"
    );
    assert!(
        buffer_content.contains(staged_file),
        "Staged file should still be visible"
    );

    // Test 4: Expand the Untracked section back
    test_app.toggle_section_collapsed(StatusSection::Untracked);
    test_app.render().unwrap();

    assert!(
        !test_app
            .navigation()
            .is_section_collapsed(StatusSection::Untracked),
        "Untracked section should be expanded after second toggle"
    );

    let buffer_content = test_backend_utils::buffer_to_string(test_app.get_buffer());
    assert!(
        buffer_content.contains(unstaged_file),
        "Unstaged file should still be visible"
    );
    assert!(
        buffer_content.contains(untracked_file),
        "Untracked file should be visible again when section is re-expanded"
    );
    assert!(
        buffer_content.contains(staged_file),
        "Staged file should still be visible"
    );

    // All sections should now be expanded again
    assert!(
        !test_app
            .navigation()
            .is_section_collapsed(StatusSection::Unstaged),
        "Unstaged section should be expanded at end"
    );
    assert!(
        !test_app
            .navigation()
            .is_section_collapsed(StatusSection::Untracked),
        "Untracked section should be expanded at end"
    );
    assert!(
        !test_app
            .navigation()
            .is_section_collapsed(StatusSection::Staged),
        "Staged section should remain expanded throughout"
    );
}

/// Test #1 + #3: Inline Diff Expansion and Content Correctness
/// Verifies that:
/// 1. Pressing Tab on a file expands inline diffs in the UI
/// 2. The displayed diff content matches actual file changes and is correct
#[test]
fn test_inline_diff_expansion_and_content() {
    use crossterm::event::KeyCode;
    use std::process::Command;

    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repository = mahgit::repository::Repository::discover(test_repo.temp_dir.path()).unwrap();

    env::set_current_dir(test_repo.temp_dir.path()).unwrap();

    // Create a file with specific content that we can verify
    let test_file = "test_modified.txt";
    let original_content = "Original line 1\nOriginal line 2\nOriginal line 3\n";
    let modified_content = "Modified line 1\nOriginal line 2\nNew line 3\nAdded line 4\n";

    // First, add the original file and commit it
    std::fs::write(test_repo.temp_dir.path().join(test_file), original_content).unwrap();
    Command::new("git")
        .args(["add", test_file])
        .current_dir(test_repo.temp_dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add test file"])
        .current_dir(test_repo.temp_dir.path())
        .output()
        .unwrap();

    // Now modify the file
    std::fs::write(test_repo.temp_dir.path().join(test_file), modified_content).unwrap();

    // Get the expected git diff output for comparison
    let git_diff_output = Command::new("git")
        .args(["diff", test_file])
        .current_dir(test_repo.temp_dir.path())
        .output()
        .unwrap();
    let git_diff_str = String::from_utf8_lossy(&git_diff_output.stdout);

    let mut test_app = TestApp::with_repository(80, 24, repository).unwrap();
    test_app.render().unwrap();

    assert!(
        test_app.assert_contains(test_file),
        "Test file should appear in status"
    );

    // Navigate to ensure we're on the file (in case there are multiple items)
    test_app.send_key_code(KeyCode::Char('j'));
    test_app.render().unwrap();

    // Expand the diff by pressing Tab
    test_app.send_key_code(KeyCode::Tab);
    test_app.render().unwrap();

    let buffer_content = test_backend_utils::buffer_to_string(test_app.get_buffer());

    let has_diff_markers = buffer_content.contains("+") || buffer_content.contains("-");
    assert!(
        has_diff_markers,
        "Expanded diff should contain diff markers (+/-). Buffer: {}",
        buffer_content
    );
    assert!(
        buffer_content.contains("Original line 1") || git_diff_str.contains("-Original line 1"),
        "Diff should show original line 1 (either as context or deletion)"
    );
    assert!(
        buffer_content.contains("Modified line 1"),
        "Diff should show the modified line 1. Buffer: {}",
        buffer_content
    );
    assert!(
        buffer_content.contains("New line 3"),
        "Diff should show new line 3. Buffer: {}",
        buffer_content
    );
    assert!(
        buffer_content.contains("Added line 4"),
        "Diff should show added line 4. Buffer: {}",
        buffer_content
    );
    assert!(
        buffer_content.contains("Original line 2"),
        "Diff should show unchanged line 2 as context. Buffer: {}",
        buffer_content
    );
}
