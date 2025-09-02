pub mod console;
pub mod feedback;
pub mod input;
pub mod navigation;
pub mod status_view;

use crate::operations::StagingOperations;
use crate::repository::Repository;
use crate::status::RepositoryStatus;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use feedback::FeedbackManager;
use input::{Command, InputHandler};
use navigation::{NavigationState, OperationContext};
use ratatui::{Terminal, backend::CrosstermBackend};
use status_view::StatusView;
use std::io::{Stdout, stdout};

pub struct App {
    should_quit: bool,
    #[allow(dead_code)]
    current_view: ViewType,
    repository: Repository,
    status: RepositoryStatus,
    navigation: NavigationState,
    input_handler: InputHandler,
    feedback_manager: FeedbackManager,
    show_help: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ViewType {
    Status,
}

impl App {
    pub fn new(repository: Repository, status: RepositoryStatus) -> Self {
        let navigation = NavigationState::new(&status);
        Self {
            should_quit: false,
            current_view: ViewType::Status,
            repository,
            status,
            navigation,
            input_handler: InputHandler::new(),
            feedback_manager: FeedbackManager::new(),
            show_help: false,
        }
    }

    pub fn is_showing_help(&self) -> bool {
        self.show_help
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Force refresh the repository status (for testing purposes)
    pub fn force_refresh(&mut self) -> Result<(), crate::repository::RepositoryError> {
        self.status.reload(&self.repository)?;
        self.navigation.update_status(&self.status);
        Ok(())
    }

    pub fn process_key_event(&mut self, key_event: crossterm::event::KeyEvent) {
        let command = self.input_handler.handle_key(key_event);
        self.handle_command(command);
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_event_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn run_event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if event::poll(std::time::Duration::from_millis(16))?
                && let Event::Key(key) = event::read()?
            {
                let command = self.input_handler.handle_key(key);
                self.handle_command(command);
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn handle_command(&mut self, command: Command) {
        match command {
            Command::MoveUp => {
                self.navigation.move_up();
            }
            Command::MoveDown => {
                self.navigation.move_down();
            }
            Command::MoveToTop => {
                self.navigation.move_to_top();
            }
            Command::MoveToBottom => {
                self.navigation.move_to_bottom();
            }
            Command::Quit | Command::ForceQuit => {
                self.should_quit = true;
            }
            Command::RefreshStatus => {
                self.refresh_status();
            }
            Command::ShowHelp => {
                self.show_help = !self.show_help;
            }
            Command::StageFile => {
                self.stage_selected_file();
            }
            Command::UnstageFile => {
                self.unstage_selected_file();
            }
            Command::AddUntracked => {
                self.add_selected_file();
            }
            Command::ToggleStage => {
                self.toggle_stage_selected_file();
            }
            Command::Unknown => {
                // Ignore unknown commands
            }
        }
    }

    fn refresh_status(&mut self) {
        if let Err(err) = self.status.reload(&self.repository) {
            self.feedback_manager
                .show_result(crate::operations::OperationResult::new(format!(
                    "Failed to refresh status: {}",
                    err
                )));
        } else {
            self.navigation.update_status(&self.status);
            self.feedback_manager
                .show_result(crate::operations::OperationResult::new(
                    "Status refreshed".to_string(),
                ));
        }
    }

    fn execute_staging_operation<F>(&mut self, operation: F, operation_name: &str)
    where
        F: FnOnce(
            &StagingOperations,
            &str,
        )
            -> Result<crate::operations::OperationResult, crate::repository::RepositoryError>,
    {
        if let Some(selected_file) = self.navigation.get_selected_file(&self.status) {
            let staging_ops = StagingOperations::new(&self.repository);
            match operation(&staging_ops, &selected_file.path) {
                Ok(result) => {
                    self.feedback_manager.show_result(result);
                    self.refresh_status_after_operation();
                }
                Err(err) => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(format!(
                            "Failed to {} {}: {}",
                            operation_name, selected_file.path, err
                        )));
                }
            }
        }
    }

    fn stage_selected_file(&mut self) {
        self.execute_staging_operation(|ops, path| ops.stage_file(path), "stage");
    }

    fn unstage_selected_file(&mut self) {
        self.execute_staging_operation(|ops, path| ops.unstage_file(path), "unstage");
    }

    fn add_selected_file(&mut self) {
        self.execute_staging_operation(|ops, path| ops.add_untracked_file(path), "add");
    }

    fn toggle_stage_selected_file(&mut self) {
        let operation_context = self.navigation.get_operation_context();
        match operation_context {
            OperationContext::CanStage => self.stage_selected_file(),
            OperationContext::CanUnstage => self.unstage_selected_file(),
            OperationContext::CanAdd => self.add_selected_file(),
            OperationContext::ReadOnly => {
                self.feedback_manager
                    .show_result(crate::operations::OperationResult::new(
                        "Cannot modify conflicted files".to_string(),
                    ));
            }
        }
    }

    fn refresh_status_after_operation(&mut self) {
        if self.status.reload(&self.repository).is_err() {
            // If reload fails, we still want to continue, just won't have updated status
        } else {
            self.navigation.update_status(&self.status);
        }
    }

    pub fn render(&self, f: &mut ratatui::Frame) {
        let status_view = StatusView::new(&self.status, &self.navigation);
        status_view.render(f, f.area());

        // Render feedback message if there is one
        if let Some(feedback) = self.feedback_manager.get_current_message() {
            self.render_feedback_message(f, feedback);
        }

        if self.show_help {
            self.render_help_overlay(f);
        }
    }

    fn render_feedback_message(
        &self,
        f: &mut ratatui::Frame,
        feedback: &crate::operations::OperationResult,
    ) {
        use ratatui::{
            layout::{Alignment, Rect},
            style::{Color, Style},
            widgets::{Block, Borders, Clear, Paragraph},
        };

        let area = f.area();

        // Create a small area at the bottom for the feedback
        let feedback_area = Rect {
            x: 0,
            y: area.height.saturating_sub(3),
            width: area.width,
            height: 3,
        };

        // Clear the area
        f.render_widget(Clear, feedback_area);

        // Create the feedback message
        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .style(Style::default().bg(Color::Rgb(20, 20, 20)));

        let paragraph = Paragraph::new(feedback.message.as_str())
            .block(block)
            .alignment(Alignment::Left)
            .style(Style::default().fg(Color::White));

        f.render_widget(paragraph, feedback_area);
    }

    fn render_help_overlay(&self, f: &mut ratatui::Frame) {
        use ratatui::{
            layout::Margin,
            style::Style,
            widgets::{Block, Borders, Clear, List, ListItem},
        };

        let area = f.area();
        let help_text = InputHandler::get_help_text();

        // Calculate height needed for help content (plus borders and title)
        let help_height = (help_text.len() as u16)
            .min(area.height.saturating_sub(2))
            .max(5);

        // Create a bottom panel that slides up from the bottom
        let help_area = ratatui::layout::Rect {
            x: 0,
            y: area.height.saturating_sub(help_height + 2), // +2 for borders
            width: area.width,
            height: help_height + 2,
        };

        // Clear the area where help will be rendered
        f.render_widget(Clear, help_area);

        let help_items: Vec<ListItem> = help_text.into_iter().map(ListItem::new).collect();

        let help_block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT) // No bottom border for slide-up effect
            .title(" Help - Press '?' to close ")
            .style(Style::default()); // No background color, use terminal default

        // Render the block first
        f.render_widget(&help_block, help_area);

        // Calculate the inner area with horizontal margin
        let inner_area = help_block.inner(help_area);
        let content_area = inner_area.inner(Margin {
            horizontal: 1,
            vertical: 0,
        });

        // Render the list content in the margin-adjusted area
        let help_list = List::new(help_items);
        f.render_widget(help_list, content_area);
    }
}
