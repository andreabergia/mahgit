use super::Modal;
use crate::ui::input::Command;

/// A simple confirmation modal for destructive operations
///
/// Shows a yes/no prompt for confirming operations like discarding changes.
pub struct ConfirmModal {
    message: String,
    confirm_command: Command,
}

impl ConfirmModal {
    /// Create a new confirmation modal
    ///
    /// # Arguments
    /// * `message` - The message to display to the user
    /// * `confirm_command` - The command to execute if the user confirms
    pub fn new(message: String, confirm_command: Command) -> Self {
        Self {
            message,
            confirm_command,
        }
    }
}

impl Modal for ConfirmModal {
    fn title(&self) -> &str {
        "Confirm"
    }

    fn options(&self) -> Vec<(char, &str, &str)> {
        vec![('y', "yes", &self.message), ('n', "no", "Cancel operation")]
    }

    fn handle_key(&self, key: char) -> Option<Command> {
        match key {
            'y' | 'Y' => Some(self.confirm_command.clone()),
            'n' | 'N' => None, // Modal will be closed automatically
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_modal_yes() {
        let modal = ConfirmModal::new(
            "Discard changes?".to_string(),
            Command::RefreshStatus, // Using RefreshStatus as a test command
        );

        assert_eq!(modal.title(), "Confirm");
        assert_eq!(modal.options().len(), 2);

        // Test 'y' confirms
        assert_eq!(modal.handle_key('y'), Some(Command::RefreshStatus));
        assert_eq!(modal.handle_key('Y'), Some(Command::RefreshStatus));

        // Test 'n' cancels
        assert_eq!(modal.handle_key('n'), None);
        assert_eq!(modal.handle_key('N'), None);

        // Test other keys do nothing
        assert_eq!(modal.handle_key('x'), None);
    }
}
