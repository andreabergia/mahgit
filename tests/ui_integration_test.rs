use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mahgit::{repository::Repository, status::RepositoryStatus, ui::App};
use tempfile::TempDir;

fn create_test_repository() -> (TempDir, Repository) {
    let temp = TempDir::new().unwrap();
    let repo_git2 = git2::Repository::init(&temp).unwrap();

    // Create an initial commit to make it a valid repository
    let sig =
        git2::Signature::new("Test User", "test@example.com", &git2::Time::new(0, 0)).unwrap();
    std::fs::write(temp.path().join("initial.txt"), "initial").unwrap();

    let mut index = repo_git2.index().unwrap();
    index.add_path(std::path::Path::new("initial.txt")).unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo_git2.find_tree(tree_id).unwrap();
    repo_git2
        .commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .unwrap();

    let repo = Repository::discover(temp.path()).unwrap();
    (temp, repo)
}

#[test]
fn test_help_window_toggle() {
    // Create a test repository and status
    let (_temp, repository) = create_test_repository();
    let status = RepositoryStatus::empty();

    // Create the app
    let mut app = App::new(repository, status);

    // Initially help should not be showing
    assert!(
        !app.is_showing_help(),
        "Help window should not be showing initially"
    );

    // Create a '?' key event
    let help_key_event = KeyEvent {
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    // Process the key event - should open help
    app.process_key_event(help_key_event);
    assert!(
        app.is_showing_help(),
        "Help window should be showing after pressing '?'"
    );

    // Process the key event again - should close help
    app.process_key_event(help_key_event);
    assert!(
        !app.is_showing_help(),
        "Help window should be closed after pressing '?' again"
    );
}

#[test]
fn test_help_window_with_populated_status() {
    use mahgit::status::{FileEntry, FileStatus};

    // Create a test repository status with some files
    let status = RepositoryStatus {
        branch_name: "main".to_string(),
        staged: vec![FileEntry::new(
            "staged_file.txt".to_string(),
            FileStatus::Added,
        )],
        unstaged: vec![FileEntry::new(
            "modified_file.txt".to_string(),
            FileStatus::Modified,
        )],
        untracked: vec!["untracked_file.txt".to_string()],
        conflicted: vec![],
    };

    // Create the app
    let (_temp, repository) = create_test_repository();
    let mut app = App::new(repository, status);

    // Initially help should not be showing
    assert!(
        !app.is_showing_help(),
        "Help window should not be showing initially"
    );

    // Create a '?' key event
    let help_key_event = KeyEvent {
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    // Process the key event - should open help
    app.process_key_event(help_key_event);
    assert!(
        app.is_showing_help(),
        "Help window should be showing after pressing '?' with populated status"
    );

    // Process other key events while help is open
    let down_key_event = KeyEvent {
        code: KeyCode::Char('j'),
        modifiers: KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    // Process down key - help should still be showing (navigation should work independently)
    app.process_key_event(down_key_event);
    assert!(
        app.is_showing_help(),
        "Help window should still be showing after navigation key"
    );

    // Close help with '?' again
    app.process_key_event(help_key_event);
    assert!(
        !app.is_showing_help(),
        "Help window should be closed after pressing '?' again"
    );
}

#[test]
fn test_help_window_integration_with_different_keys() {
    let (_temp, repository) = create_test_repository();
    let status = RepositoryStatus::empty();
    let mut app = App::new(repository, status);

    // Test various key combinations that should NOT open help
    let test_keys = vec![
        KeyEvent {
            code: KeyCode::Char('h'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        },
        KeyEvent {
            code: KeyCode::Char('H'),
            modifiers: KeyModifiers::SHIFT,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        },
        KeyEvent {
            code: KeyCode::Char('?'),
            modifiers: KeyModifiers::CONTROL,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        },
    ];

    for key_event in test_keys {
        app.process_key_event(key_event);
        // Help should not be showing for any of these keys
        assert!(
            !app.is_showing_help(),
            "Help window should not open for key: {:?}",
            key_event
        );
    }

    // Now test the correct key combination
    let correct_help_key = KeyEvent {
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    app.process_key_event(correct_help_key);
    assert!(
        app.is_showing_help(),
        "Help window should open for the correct '?' key combination"
    );
}
