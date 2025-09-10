use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
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

    // File operations
    StageFile,
    UnstageFile,
    AddUntracked,
    ToggleStage,

    // View commands
    EnterDiffView,
    ExitDiffView,

    // Accordion commands
    ToggleAccordion, // Contextual Tab key behavior

    // Diff navigation commands
    ScrollDiffUp,
    ScrollDiffDown,
    PageDiffUp,
    PageDiffDown,
    JumpToNextHunk,
    JumpToPreviousHunk,
    NextHunk,
    PreviousHunk,
    GoToTopOfDiff,
    GoToBottomOfDiff,

    // Hunk operations
    StageHunk,
    UnstageHunk,

    // Help
    ShowHelp,

    // Unknown command
    Unknown,

    // No-op command for unhandled events
    None,
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
                code: KeyCode::Left,
                ..
            } => {
                self.clear_sequence_state();
                Command::PreviousHunk
            }

            KeyEvent {
                code: KeyCode::Right,
                ..
            } => {
                self.clear_sequence_state();
                Command::NextHunk
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

            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                self.clear_sequence_state();
                Command::ToggleAccordion
            }

            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                self.clear_sequence_state();
                Command::EnterDiffView
            }

            KeyEvent {
                code: KeyCode::PageDown,
                ..
            } => {
                self.clear_sequence_state();
                Command::PageDiffDown
            }

            KeyEvent {
                code: KeyCode::PageUp,
                ..
            } => {
                self.clear_sequence_state();
                Command::PageDiffUp
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

            // Handle space key
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.clear_sequence_state();
                Command::ToggleStage
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
                    's' => Command::StageFile,
                    'u' => Command::UnstageFile,
                    'a' => Command::AddUntracked,
                    'f' => Command::PageDiffDown,
                    'b' => Command::PageDiffUp,
                    'n' => Command::JumpToNextHunk,
                    'p' => Command::JumpToPreviousHunk,
                    'S' => Command::StageHunk,
                    'U' => Command::UnstageHunk,
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

    pub fn handle_mouse(&mut self, mouse_event: MouseEvent) -> Command {
        match mouse_event.kind {
            MouseEventKind::ScrollUp => Command::MoveUp,
            MouseEventKind::ScrollDown => Command::MoveDown,
            _ => Command::None,
        }
    }

    fn clear_sequence_state(&mut self) {
        self.previous_key = None;
    }

    fn handle_two_key_sequence(&mut self, current_char: char) -> Option<Command> {
        let now = Instant::now();
        let sequence_timeout = Duration::from_millis(1000);

        if let Some(prev) = &self.previous_key
            && now.duration_since(prev.timestamp) <= sequence_timeout
        {
            let sequence = format!("{}{}", prev.character, current_char);
            self.clear_sequence_state();

            return match sequence.as_str() {
                "gg" => Some(Command::MoveToTop),
                _ => None,
            };
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
            "  j/↓     Move down / Scroll diff down",
            "  k/↑     Move up / Scroll diff up",
            "  gg/Home Jump to top / Top of diff",
            "  G/End   Jump to bottom / Bottom of diff",
            "",
            "Section & Diff Navigation:",
            "  Tab     Toggle section collapsed/expanded",
            "  Enter   Enter diff view",
            "  f/PgDn  Page down in diff",
            "  b/PgUp  Page up in diff",
            "  n/→     Jump to next hunk",
            "  p/←     Jump to previous hunk",
            "",
            "File Operations:",
            "  s       Stage file",
            "  u       Unstage file",
            "  a       Add untracked file",
            "  Space   Toggle stage/unstage",
            "",
            "Hunk Operations (in diff view):",
            "  S       Stage current hunk",
            "  U       Unstage current hunk",
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

    #[test]
    fn test_file_operation_keys() {
        let mut handler = InputHandler::new();

        let stage_key = KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(stage_key), Command::StageFile);

        let unstage_key = KeyEvent {
            code: KeyCode::Char('u'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(unstage_key), Command::UnstageFile);

        let add_key = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(add_key), Command::AddUntracked);

        let space_key = KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(space_key), Command::ToggleStage);
    }

    #[test]
    fn test_help_key() {
        let mut handler = InputHandler::new();

        let help_key = KeyEvent {
            code: KeyCode::Char('?'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(help_key), Command::ShowHelp);
    }

    #[test]
    fn test_help_text_content() {
        let help_text = InputHandler::get_help_text();
        assert!(!help_text.is_empty());
        assert!(help_text.iter().any(|line| line.contains("?")));
        assert!(help_text.iter().any(|line| line.contains("Show this help")));
    }

    #[test]
    fn test_arrow_key_navigation() {
        let mut handler = InputHandler::new();

        let left_arrow = KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(left_arrow), Command::PreviousHunk);

        let right_arrow = KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(right_arrow), Command::NextHunk);
    }

    #[test]
    fn test_existing_hunk_navigation_keys() {
        let mut handler = InputHandler::new();

        let n_key = KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(n_key), Command::JumpToNextHunk);

        let p_key = KeyEvent {
            code: KeyCode::Char('p'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(p_key), Command::JumpToPreviousHunk);
    }

    #[test]
    fn test_mouse_scroll_up() {
        let mut handler = InputHandler::new();

        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };

        assert_eq!(handler.handle_mouse(scroll_up), Command::MoveUp);
    }

    #[test]
    fn test_mouse_scroll_down() {
        let mut handler = InputHandler::new();

        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };

        assert_eq!(handler.handle_mouse(scroll_down), Command::MoveDown);
    }

    #[test]
    fn test_mouse_unsupported_events() {
        let mut handler = InputHandler::new();

        let click = MouseEvent {
            kind: MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };

        assert_eq!(handler.handle_mouse(click), Command::None);

        let move_event = MouseEvent {
            kind: MouseEventKind::Moved,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };

        assert_eq!(handler.handle_mouse(move_event), Command::None);
    }
}
