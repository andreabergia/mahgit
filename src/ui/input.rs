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
struct KeySequenceState {
    first_g_time: Option<Instant>,
}

pub struct InputHandler {
    sequence_state: KeySequenceState,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            sequence_state: KeySequenceState { first_g_time: None },
        }
    }

    pub fn handle_key(&mut self, key_event: KeyEvent) -> Command {
        match key_event {
            // Navigation keys
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                self.clear_sequence_state();
                Command::MoveDown
            }

            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Up, ..
            } => {
                self.clear_sequence_state();
                Command::MoveUp
            }

            // Jump to top - Home key
            KeyEvent {
                code: KeyCode::Home,
                ..
            } => {
                self.clear_sequence_state();
                Command::MoveToTop
            }

            // Handle 'g' key for 'gg' sequence to jump to top
            KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let now = Instant::now();
                if let Some(first_g_time) = self.sequence_state.first_g_time {
                    // Check if this is the second 'g' within 1 second
                    if now.duration_since(first_g_time) <= Duration::from_millis(1000) {
                        self.clear_sequence_state();
                        return Command::MoveToTop;
                    }
                }
                // This is either the first 'g' or too much time has passed
                self.sequence_state.first_g_time = Some(now);
                Command::Unknown
            }

            // Jump to bottom - 'G' and End key
            KeyEvent {
                code: KeyCode::Char('G'),
                modifiers: KeyModifiers::SHIFT,
                ..
            }
            | KeyEvent {
                code: KeyCode::End, ..
            } => {
                self.clear_sequence_state();
                Command::MoveToBottom
            }

            // Application control
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.clear_sequence_state();
                Command::Quit
            }

            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.clear_sequence_state();
                Command::ForceQuit
            }

            KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.clear_sequence_state();
                Command::RefreshStatus
            }

            // Help
            KeyEvent {
                code: KeyCode::Char('?'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.clear_sequence_state();
                Command::ShowHelp
            }

            // Default case
            _ => {
                self.clear_sequence_state();
                Command::Unknown
            }
        }
    }

    fn clear_sequence_state(&mut self) {
        self.sequence_state.first_g_time = None;
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

        // Wait and simulate timeout by manually clearing state
        handler.clear_sequence_state();

        // Second 'g' after timeout should be treated as first 'g' again
        assert_eq!(handler.handle_key(g_key), Command::Unknown);
    }
}
