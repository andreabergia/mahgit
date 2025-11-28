use super::Modal;
use crate::ui::input::{Command, CommitMode};

/// Modal for commit operations
///
/// Displays available commit modes and handles key selection.
/// Supports: normal commit, amend, extend, and reword.
#[derive(Default)]
pub struct CommitModal;

impl CommitModal {
    /// Create a new commit modal
    pub fn new() -> Self {
        Self
    }
}

impl Modal for CommitModal {
    fn title(&self) -> &str {
        "Commit"
    }

    fn options(&self) -> Vec<(char, &str, &str)> {
        vec![
            ('c', "commit", "Create commit"),
            ('a', "amend", "Amend HEAD"),
            ('e', "extend", "Extend HEAD"),
            ('w', "reword", "Reword HEAD"),
        ]
    }

    fn handle_key(&self, key: char) -> Option<Command> {
        match key {
            'c' => Some(Command::Commit(CommitMode::Normal)),
            'a' => Some(Command::Commit(CommitMode::Amend)),
            'e' => Some(Command::Commit(CommitMode::Extend)),
            'w' => Some(Command::Commit(CommitMode::Reword)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_modal_options() {
        let modal = CommitModal::new();
        let options = modal.options();

        assert_eq!(options.len(), 4);
        assert_eq!(options[0], ('c', "commit", "Create commit"));
        assert_eq!(options[1], ('a', "amend", "Amend HEAD"));
    }

    #[test]
    fn test_commit_modal_key_handling() {
        let modal = CommitModal::new();

        assert_eq!(
            modal.handle_key('c'),
            Some(Command::Commit(CommitMode::Normal))
        );
        assert_eq!(
            modal.handle_key('a'),
            Some(Command::Commit(CommitMode::Amend))
        );
        assert_eq!(
            modal.handle_key('e'),
            Some(Command::Commit(CommitMode::Extend))
        );
        assert_eq!(
            modal.handle_key('w'),
            Some(Command::Commit(CommitMode::Reword))
        );
        assert_eq!(modal.handle_key('x'), None);
    }
}
