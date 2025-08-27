use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // Navigation commands
    MoveUp,
    MoveDown,
    MoveToTop,
    MoveToBottom,

    // Application commands
    Quit,
    ForceQuit,
    RefreshStatus,

    // Help
    ShowHelp,

    // Unknown command
    Unknown,
}

#[derive(Debug)]
struct PreviousKey {
    character: char,
    timestamp: Instant,
}

pub struct InputHandler {
    previous_key: Option<PreviousKey>,
}

impl InputHandler {
    pub fn new() -> Self {
        Self { previous_key: None }
    }

    pub fn handle_key(&mut self, key_event: KeyEvent) -> Command {
        match key_event {
            // Arrow keys and special keys
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                self.clear_sequence_state();
                Command::MoveDown
            }

            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                self.clear_sequence_state();
                Command::MoveUp
            }

            KeyEvent {
                code: KeyCode::Home,
                ..
            } => {
                self.clear_sequence_state();
                Command::MoveToTop
            }

            KeyEvent {
                code: KeyCode::End, ..
            } => {
                self.clear_sequence_state();
                Command::MoveToBottom
            }

            // Control sequences
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.clear_sequence_state();
                Command::ForceQuit
            }

            // Shift+char sequences
            KeyEvent {
                code: KeyCode::Char('G'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.clear_sequence_state();
                Command::MoveToBottom
            }

            // Handle all regular characters
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Check if this forms a two-key sequence
                if let Some(command) = self.handle_two_key_sequence(c) {
                    return command;
                }

                // If it's not a sequence, check for single-key commands
                let command = match c {
                    'j' => Command::MoveDown,
                    'k' => Command::MoveUp,
                    'q' => Command::Quit,
                    'r' => Command::RefreshStatus,
                    '?' => Command::ShowHelp,
                    _ => Command::Unknown,
                };

                // Only clear sequence state if we found a single-key command
                if !matches!(command, Command::Unknown) {
                    self.clear_sequence_state();
                }

                command
            }

            // Default case
            _ => {
                self.clear_sequence_state();
                Command::Unknown
            }
        }
    }

    fn clear_sequence_state(&mut self) {
        self.previous_key = None;
    }

    fn handle_two_key_sequence(&mut self, current_char: char) -> Option<Command> {
        let now = Instant::now();
        let sequence_timeout = Duration::from_millis(1000);

        if let Some(prev) = &self.previous_key {
            if now.duration_since(prev.timestamp) <= sequence_timeout {
                let sequence = format!("{}{}", prev.character, current_char);
                self.clear_sequence_state();

                return match sequence.as_str() {
                    "gg" => Some(Command::MoveToTop),
                    _ => None,
                };
            }
        }

        // Store this key as the potential first key of a sequence
        self.previous_key = Some(PreviousKey {
            character: current_char,
            timestamp: now,
        });
        None
    }

    pub fn get_help_text() -> Vec<&'static str> {
        vec![
            "Navigation:",
            "  j/↓     Move down",
            "  k/↑     Move up",
            "  gg/Home Jump to top",
            "  G/End   Jump to bottom",
            "",
            "Application:",
            "  q       Quit application",
            "  C-c     Force quit",
            "  r       Refresh repository status",
            "",
            "Help:",
            "  ?       Show this help",
        ]
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_keys() {
        let mut handler = InputHandler::new();

        let down_j = KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(down_j), Command::MoveDown);

        let up_k = KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(up_k), Command::MoveUp);
    }

    #[test]
    fn test_quit_commands() {
        let mut handler = InputHandler::new();

        let quit = KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(quit), Command::Quit);

        let force_quit = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(force_quit), Command::ForceQuit);
    }

    #[test]
    fn test_gg_sequence() {
        let mut handler = InputHandler::new();

        let g_key = KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        // First 'g' should return Unknown (waiting for second 'g')
        assert_eq!(handler.handle_key(g_key), Command::Unknown);

        // Second 'g' should return MoveToTop
        assert_eq!(handler.handle_key(g_key), Command::MoveToTop);
    }

    #[test]
    fn test_gg_timeout() {
        let mut handler = InputHandler::new();

        let g_key = KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        // First 'g'
        assert_eq!(handler.handle_key(g_key), Command::Unknown);

        // Simulate timeout by manually clearing state (represents passage of time)
        handler.clear_sequence_state();

        // Second 'g' after timeout should be treated as first 'g' again
        assert_eq!(handler.handle_key(g_key), Command::Unknown);
    }

    #[test]
    fn test_generic_sequence_handling() {
        let mut handler = InputHandler::new();

        // Test that non-sequence keys still work correctly
        let j_key = KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(j_key), Command::MoveDown);

        // Test that unknown sequences return Unknown for both keys
        let x_key = KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        let y_key = KeyEvent {
            code: KeyCode::Char('y'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };

        // First key of unknown sequence
        assert_eq!(handler.handle_key(x_key), Command::Unknown);
        // Second key of unknown sequence
        assert_eq!(handler.handle_key(y_key), Command::Unknown);
    }
}
