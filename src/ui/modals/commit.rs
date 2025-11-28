use super::{Modal, render_bottom_panel_modal};
use crate::config::Config;
use crate::ui::input::{Command, CommitMode};
use ratatui::{Frame, layout::Rect};
use std::sync::Arc;

/// Modal for commit operations
///
/// Displays available commit modes and handles key selection.
/// Supports: normal commit, amend, extend, reword, and no-verify.
pub struct CommitModal {
    #[allow(dead_code)] // Reserved for future customization
    config: Arc<Config>,
}

impl CommitModal {
    /// Create a new commit modal
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
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
            ('n', "no-verify", "Skip hooks"),
        ]
    }

    fn handle_key(&self, key: char) -> Option<Command> {
        match key {
            'c' => Some(Command::Commit(CommitMode::Normal)),
            'a' => Some(Command::Commit(CommitMode::Amend)),
            'e' => Some(Command::Commit(CommitMode::Extend)),
            'w' => Some(Command::Commit(CommitMode::Reword)),
            'n' => Some(Command::Commit(CommitMode::NoVerify)),
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, config: &Config) {
        // Use the default bottom-panel rendering
        render_bottom_panel_modal(frame, area, config, self.title(), &self.options());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn test_config() -> Arc<Config> {
        Arc::new(Config {
            theme: Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        })
    }

    #[test]
    fn test_commit_modal_options() {
        let modal = CommitModal::new(test_config());
        let options = modal.options();

        assert_eq!(options.len(), 5);
        assert_eq!(options[0], ('c', "commit", "Create commit"));
        assert_eq!(options[1], ('a', "amend", "Amend HEAD"));
    }

    #[test]
    fn test_commit_modal_key_handling() {
        let modal = CommitModal::new(test_config());

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
        assert_eq!(
            modal.handle_key('n'),
            Some(Command::Commit(CommitMode::NoVerify))
        );
        assert_eq!(modal.handle_key('x'), None);
    }
}
