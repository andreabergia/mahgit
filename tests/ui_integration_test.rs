use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mahgit::{status::RepositoryStatus, ui::App};

#[test]
fn test_help_window_toggle() {
    // Create a test repository status
    let status = RepositoryStatus::empty();

    // Create the app
    let mut app = App::new(status);

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
    let mut app = App::new(status);

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
    let status = RepositoryStatus::empty();
    let mut app = App::new(status);

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
