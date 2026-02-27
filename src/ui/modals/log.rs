use super::Modal;
use crate::ui::input::Command;

/// Modal for log operations
///
/// Displays available log modes and handles key selection.
#[derive(Default)]
pub struct LogModal;

impl LogModal {
    pub fn new() -> Self {
        Self
    }
}

impl Modal for LogModal {
    fn title(&self) -> &str {
        "Log"
    }

    fn options(&self) -> Vec<(char, &str, &str)> {
        vec![('l', "log", "Log current branch")]
    }

    fn handle_key(&self, key: char) -> Option<Command> {
        match key {
            'l' => Some(Command::OpenLog),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_modal_options() {
        let modal = LogModal::new();
        let options = modal.options();

        assert_eq!(options.len(), 1);
        assert_eq!(options[0], ('l', "log", "Log current branch"));
    }

    #[test]
    fn test_log_modal_key_handling() {
        let modal = LogModal::new();

        assert_eq!(modal.handle_key('l'), Some(Command::OpenLog));
        assert_eq!(modal.handle_key('x'), None);
    }
}
