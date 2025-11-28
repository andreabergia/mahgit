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

    // View commands
    EnterDiffView,
    ExitDiffView,
    OpenInEditor,

    // Accordion commands
    ToggleAccordion, // Contextual Tab key behavior

    // Diff navigation commands
    ScrollDiffUp,
    ScrollDiffDown,
    PageDiffUp,
    PageDiffDown,
    PageForward,
    JumpToNextHunk,
    JumpToPreviousHunk,
    NextHunk,
    PreviousHunk,
    ScrollViewportDown,
    ScrollViewportUp,
    GoToTopOfDiff,
    GoToBottomOfDiff,

    // Hierarchy navigation commands
    MoveUpHierarchy,   // Left: hunk -> file, file stays at file
    MoveDownHierarchy, // Right: file -> first hunk (if expanded), hunk stays at hunk

    // Help
    ShowHelp,
    CloseHelp,

    // Hunk context expansion
    IncreaseHunkContext,
    DecreaseHunkContext,

    // Commit operations
    OpenCommitModal,
    Commit(CommitMode),

    // Unknown command
    Unknown,

    // No-op command for unhandled events
    None,
}

/// Commit operation modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitMode {
    /// Normal commit (git commit)
    Normal,
    /// Amend the last commit (git commit --amend)
    Amend,
    /// Extend the last commit without editing message (git commit --amend --no-edit)
    Extend,
    /// Reword the last commit (change message only)
    Reword,
    /// Commit without running pre-commit hooks (git commit --no-verify)
    NoVerify,
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
                Command::ScrollDiffDown
            }

            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                self.clear_sequence_state();
                Command::ScrollDiffUp
            }

            KeyEvent {
                code: KeyCode::Left,
                ..
            } => {
                self.clear_sequence_state();
                Command::MoveUpHierarchy
            }

            KeyEvent {
                code: KeyCode::Right,
                ..
            } => {
                self.clear_sequence_state();
                Command::MoveDownHierarchy
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
                Command::OpenInEditor
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

            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.clear_sequence_state();
                Command::CloseHelp
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
                Command::PageForward
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
                    'j' => Command::ScrollDiffDown,
                    'k' => Command::ScrollDiffUp,
                    'q' => Command::Quit,
                    'r' => Command::RefreshStatus,
                    's' => Command::StageFile,
                    'u' => Command::UnstageFile,
                    'a' => Command::AddUntracked,
                    'f' => Command::PageDiffDown,
                    'b' => Command::PageDiffUp,
                    'n' => Command::JumpToNextHunk,
                    'p' => Command::JumpToPreviousHunk,
                    '?' => Command::ShowHelp,
                    '=' => Command::IncreaseHunkContext,
                    '-' => Command::DecreaseHunkContext,
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
            MouseEventKind::ScrollUp => Command::ScrollViewportUp,
            MouseEventKind::ScrollDown => Command::ScrollViewportDown,
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
            "  ↓/j     Move selection down",
            "  ↑/k     Move selection up",
            "  ←       Move up hierarchy (hunk → file, collapse diff)",
            "  →       Move down hierarchy (expand/enter diff)",
            "  PgUp/b  Scroll up a page",
            "  PgDn/f  Scroll down a page",
            "  Space   Page forward",
            "  gg/Home Jump to top",
            "  G/End   Jump to bottom",
            "  wheel   Scroll viewport",
            "",
            "Section & Diff Navigation:",
            "  Tab     Collapse/expand the selected file's inline diff",
            "  Enter   Open file in editor",
            "  n       Jump to end of current file/section",
            "  p       Jump to start of current file/section",
            "  =       Expand context in current hunk",
            "  -       Reduce context in current hunk",
            "",
            "File Operations:",
            "  s       Stage file or current diff hunk",
            "  u       Unstage file or current diff hunk",
            "  a       Add untracked file",
            "",
            "Application:",
            "  q       Quit application",
            "  C-c     Force quit",
            "  r       Refresh repository status",
            "",
            "Help:",
            "  ?       Toggle help",
            "  Esc     Close help",
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
        assert_eq!(handler.handle_key(down_j), Command::ScrollDiffDown);

        let up_k = KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(up_k), Command::ScrollDiffUp);
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
        assert_eq!(handler.handle_key(j_key), Command::ScrollDiffDown);

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
        assert_eq!(handler.handle_key(space_key), Command::PageForward);
    }

    #[test]
    fn test_shift_s_u_mapping() {
        let mut handler = InputHandler::new();
        let shift_s = KeyEvent {
            code: KeyCode::Char('S'),
            modifiers: KeyModifiers::SHIFT,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let shift_u = KeyEvent {
            code: KeyCode::Char('U'),
            modifiers: KeyModifiers::SHIFT,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(shift_s), Command::Unknown);
        assert_eq!(handler.handle_key(shift_u), Command::Unknown);
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
        assert!(help_text.iter().any(|line| line.contains("Toggle help")));
        assert!(help_text.iter().any(|line| line.contains("Close help")));
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
        assert_eq!(handler.handle_key(left_arrow), Command::MoveUpHierarchy);

        let right_arrow = KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(right_arrow), Command::MoveDownHierarchy);
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

        assert_eq!(handler.handle_mouse(scroll_up), Command::ScrollViewportUp);
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

        assert_eq!(
            handler.handle_mouse(scroll_down),
            Command::ScrollViewportDown
        );
    }

    #[test]
    fn test_arrow_vertical_scroll_commands() {
        let mut handler = InputHandler::new();

        let down_arrow = KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(down_arrow), Command::ScrollDiffDown);

        let up_arrow = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(handler.handle_key(up_arrow), Command::ScrollDiffUp);
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
