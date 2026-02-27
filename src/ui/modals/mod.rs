pub mod commit;
pub mod confirm;
pub mod log;

pub use commit::CommitModal;
pub use confirm::ConfirmModal;
pub use log::LogModal;

use crate::config::Config;
use crate::ui::input::Command;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Represents the type of modal currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalContext {
    None,
    Commit,
    Confirm,
    Log,
}

/// Trait for implementing modal dialogs
///
/// Modals are bottom-panel overlays that present a set of options to the user.
/// They support keyboard-driven selection and consistent rendering.
pub trait Modal {
    /// Get the modal title displayed at the top of the panel
    fn title(&self) -> &str;

    /// Get the available options as (key, short_label, description) tuples
    ///
    /// - key: The character to press to select this option
    /// - short_label: A brief label for the option (e.g., "commit", "amend")
    /// - description: A longer description of what the option does
    fn options(&self) -> Vec<(char, &str, &str)>;

    /// Handle a key press and return the command to execute, if any
    ///
    /// Returns None if the key doesn't match any option.
    fn handle_key(&self, key: char) -> Option<Command>;

    /// Render the modal in the given frame
    ///
    /// The default implementation provides a bottom-panel style consistent
    /// with the help overlay. Custom implementations can override this.
    fn render(&self, frame: &mut Frame, area: Rect, config: &Config) {
        render_bottom_panel_modal(frame, area, config, self.title(), &self.options());
    }
}

/// Render a bottom-panel modal with consistent styling
///
/// This function provides a reusable rendering implementation for modals
/// that display as bottom panels, similar to the help overlay.
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The full terminal area
/// * `config` - The application config (for theme access)
/// * `title` - The modal title
/// * `options` - The available options as (key, label, description) tuples
pub fn render_bottom_panel_modal(
    frame: &mut Frame,
    area: Rect,
    config: &Config,
    title: &str,
    options: &[(char, &str, &str)],
) {
    let theme = &config.theme;

    // Calculate height: 2 for borders + 1 per option + 1 for padding
    let modal_height = (options.len() as u16) + 3;
    let modal_height = modal_height.min(area.height - 2); // Leave room for margin

    // Position at bottom of screen
    let chunks =
        Layout::vertical([Constraint::Min(0), Constraint::Length(modal_height)]).split(area);

    let modal_area = chunks[1];

    // Clear the background
    frame.render_widget(Clear, modal_area);

    // Create the modal block with borders (matching help overlay style)
    let block = Block::default()
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .title(format!(" {} ", title))
        .style(Style::default()); // Use terminal default background

    // Build the content lines
    let mut lines = Vec::new();

    for (key, label, description) in options {
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                key.to_string(),
                Style::default()
                    .fg(theme.help_key)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{:<15}", label),
                Style::default().fg(theme.help_key),
            ),
            Span::raw(" "),
            Span::styled(*description, Style::default().fg(theme.help_desc)),
        ]);
        lines.push(line);
    }

    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, modal_area);
}

/// Box wrapper for Modal trait objects
///
/// This allows storing different modal types in the same field.
pub type BoxedModal = Box<dyn Modal>;

/// Create a boxed modal from any type implementing Modal
pub fn boxed<M: Modal + 'static>(modal: M) -> BoxedModal {
    Box::new(modal)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestModal;

    impl Modal for TestModal {
        fn title(&self) -> &str {
            "Test Modal"
        }

        fn options(&self) -> Vec<(char, &str, &str)> {
            vec![
                ('a', "option-a", "Description A"),
                ('b', "option-b", "Description B"),
            ]
        }

        fn handle_key(&self, key: char) -> Option<Command> {
            match key {
                'a' => Some(Command::RefreshStatus),
                'b' => Some(Command::Quit),
                _ => None,
            }
        }
    }

    #[test]
    fn test_modal_basic() {
        let modal = TestModal;
        assert_eq!(modal.title(), "Test Modal");
        assert_eq!(modal.options().len(), 2);
        assert_eq!(modal.handle_key('a'), Some(Command::RefreshStatus));
        assert_eq!(modal.handle_key('b'), Some(Command::Quit));
        assert_eq!(modal.handle_key('c'), None);
    }
}
