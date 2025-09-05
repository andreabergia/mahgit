pub mod console;
pub mod diff_view;
pub mod feedback;
pub mod input;
pub mod navigation;
pub mod status_view;

use crate::diff::{Diff, DiffGenerator};
use crate::operations::{HunkStager, StagingOperations};
use crate::repository::Repository;
use crate::status::RepositoryStatus;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use feedback::FeedbackManager;
use input::{Command, InputHandler};
use navigation::{NavigationState, OperationContext, SelectedFile};
use ratatui::{Terminal, backend::CrosstermBackend};
use status_view::StatusView;
use std::cell::RefCell;
use std::io::{Stdout, stdout};

pub struct App {
    should_quit: bool,
    current_view: ViewType,
    repository: Repository,
    status: RepositoryStatus,
    navigation: NavigationState,
    input_handler: InputHandler,
    feedback_manager: FeedbackManager,
    show_help: bool,
    current_diff: Option<Diff>,
    diff_view: Option<RefCell<diff_view::DiffView>>,
}

#[derive(Clone, PartialEq)]
pub enum ViewType {
    Status,
    Diff(DiffViewState),
}

#[derive(Clone, PartialEq)]
pub struct DiffViewState {
    pub file_path: String,
    pub diff_context: crate::diff::DiffContext,
    pub scroll_position: usize,
    pub hunk_index: Option<usize>,
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
            current_diff: None,
            diff_view: None,
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
            Command::MoveUp => match self.current_view {
                ViewType::Status => self.navigation.move_up(),
                ViewType::Diff(_) => self.scroll_diff_up(),
            },
            Command::MoveDown => match self.current_view {
                ViewType::Status => self.navigation.move_down(),
                ViewType::Diff(_) => self.scroll_diff_down(),
            },
            Command::MoveToTop => match self.current_view {
                ViewType::Status => self.navigation.move_to_top(),
                ViewType::Diff(_) => self.go_to_top_of_diff(),
            },
            Command::MoveToBottom => match self.current_view {
                ViewType::Status => self.navigation.move_to_bottom(),
                ViewType::Diff(_) => self.go_to_bottom_of_diff(),
            },
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
            Command::EnterDiffView => {
                self.toggle_diff_view();
            }
            Command::ExitDiffView => {
                self.exit_diff_view();
            }
            Command::ScrollDiffUp => {
                self.scroll_diff_up();
            }
            Command::ScrollDiffDown => {
                self.scroll_diff_down();
            }
            Command::PageDiffUp => {
                self.page_diff_up();
            }
            Command::PageDiffDown => {
                self.page_diff_down();
            }
            Command::JumpToNextHunk => {
                self.jump_to_next_hunk();
            }
            Command::JumpToPreviousHunk => {
                self.jump_to_previous_hunk();
            }
            Command::NextHunk => {
                self.navigate_to_next_hunk();
            }
            Command::PreviousHunk => {
                self.navigate_to_previous_hunk();
            }
            Command::GoToTopOfDiff => {
                self.go_to_top_of_diff();
            }
            Command::GoToBottomOfDiff => {
                self.go_to_bottom_of_diff();
            }
            Command::StageHunk => {
                self.stage_current_hunk();
            }
            Command::UnstageHunk => {
                self.unstage_current_hunk();
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

    fn toggle_diff_view(&mut self) {
        match &self.current_view {
            ViewType::Status => self.enter_diff_view(),
            ViewType::Diff(_) => self.exit_diff_view(),
        }
    }

    fn enter_diff_view(&mut self) {
        if !matches!(self.current_view, ViewType::Status) {
            return; // Only allow entering diff view from status view
        }

        if let Some(selected_file) = self.navigation.get_selected_file(&self.status) {
            // Determine the appropriate diff context based on the file's location
            let diff_context = self.determine_diff_context(&selected_file);

            // Generate the diff
            let diff_generator = DiffGenerator::new(self.repository.git2_repo());
            match diff_generator.generate_diff(&selected_file.path, diff_context.clone()) {
                Ok(diff) => {
                    self.diff_view = Some(RefCell::new(diff_view::DiffView::new(diff.clone())));
                    self.current_diff = Some(diff);
                    self.current_view = ViewType::Diff(DiffViewState {
                        file_path: selected_file.path.clone(),
                        diff_context,
                        scroll_position: 0,
                        hunk_index: None,
                    });
                }
                Err(err) => {
                    // For certain recoverable errors, show the error in diff view instead of feedback
                    match &err {
                        crate::diff::generator::DiffError::BinaryFile(_)
                        | crate::diff::generator::DiffError::FileTooLarge(_, _)
                        | crate::diff::generator::DiffError::TerminalCompatibility(_) => {
                            self.diff_view =
                                Some(RefCell::new(diff_view::DiffView::new_with_error(
                                    selected_file.path.clone(),
                                    err,
                                )));
                            self.current_view = ViewType::Diff(DiffViewState {
                                file_path: selected_file.path.clone(),
                                diff_context,
                                scroll_position: 0,
                                hunk_index: None,
                            });
                        }
                        _ => {
                            // For other errors, show in feedback
                            self.feedback_manager.show_result(
                                crate::operations::OperationResult::new(format!(
                                    "Failed to generate diff for {}: {}",
                                    selected_file.path, err
                                )),
                            );
                        }
                    }
                }
            }
        }
    }

    fn exit_diff_view(&mut self) {
        if matches!(self.current_view, ViewType::Diff(_)) {
            self.current_view = ViewType::Status;
            self.current_diff = None;
            self.diff_view = None;
        }
    }

    fn determine_diff_context(&self, file: &SelectedFile) -> crate::diff::DiffContext {
        use navigation::FileContext;

        match file.context {
            FileContext::Staged => crate::diff::DiffContext::IndexToHead,
            FileContext::Unstaged => crate::diff::DiffContext::WorkingTreeToIndex,
            FileContext::Untracked => crate::diff::DiffContext::WorkingTreeToIndex,
            FileContext::Conflicted => crate::diff::DiffContext::WorkingTreeToHead,
        }
    }

    pub fn render(&self, f: &mut ratatui::Frame) {
        let area = f.area();

        match &self.current_view {
            ViewType::Status => {
                let status_view = StatusView::new(&self.status, &self.navigation);
                status_view.render(f, area);
            }
            ViewType::Diff(_diff_state) => {
                if let Some(diff_view_cell) = &self.diff_view {
                    let mut diff_view = diff_view_cell.borrow_mut();
                    diff_view.update_viewport_height(area.height);
                    diff_view.render(f, area);
                }
            }
        }

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

    // Diff navigation methods
    fn scroll_diff_up(&mut self) {
        if let Some(diff_view_cell) = &self.diff_view {
            let mut diff_view = diff_view_cell.borrow_mut();
            diff_view.scroll_up(1);
        }
    }

    fn scroll_diff_down(&mut self) {
        if let Some(diff_view_cell) = &self.diff_view {
            let mut diff_view = diff_view_cell.borrow_mut();
            diff_view.scroll_down(1);
        }
    }

    fn page_diff_up(&mut self) {
        if let Some(diff_view_cell) = &self.diff_view {
            let mut diff_view = diff_view_cell.borrow_mut();
            diff_view.page_up();
        }
    }

    fn page_diff_down(&mut self) {
        if let Some(diff_view_cell) = &self.diff_view {
            let mut diff_view = diff_view_cell.borrow_mut();
            diff_view.page_down();
        }
    }

    fn jump_to_next_hunk(&mut self) {
        if let Some(diff_view_cell) = &self.diff_view {
            let mut diff_view = diff_view_cell.borrow_mut();
            diff_view.jump_to_next_hunk();
        }
    }

    fn jump_to_previous_hunk(&mut self) {
        if let Some(diff_view_cell) = &self.diff_view {
            let mut diff_view = diff_view_cell.borrow_mut();
            diff_view.jump_to_previous_hunk();
        }
    }

    fn navigate_to_next_hunk(&mut self) {
        if let Some(diff_view_cell) = &self.diff_view {
            let mut diff_view = diff_view_cell.borrow_mut();
            diff_view.navigate_to_next_hunk();
        }
    }

    fn navigate_to_previous_hunk(&mut self) {
        if let Some(diff_view_cell) = &self.diff_view {
            let mut diff_view = diff_view_cell.borrow_mut();
            diff_view.navigate_to_previous_hunk();
        }
    }

    fn go_to_top_of_diff(&mut self) {
        if let Some(diff_view_cell) = &self.diff_view {
            let mut diff_view = diff_view_cell.borrow_mut();
            diff_view.go_to_top();
        }
    }

    fn go_to_bottom_of_diff(&mut self) {
        if let Some(diff_view_cell) = &self.diff_view {
            let mut diff_view = diff_view_cell.borrow_mut();
            diff_view.go_to_bottom();
        }
    }

    fn stage_current_hunk(&mut self) {
        // Extract the required data first to avoid borrowing issues
        let (hunk_info, file_path) = {
            if let (Some(diff), ViewType::Diff(diff_state)) =
                (&self.current_diff, &self.current_view)
            {
                if let Some(diff_view_cell) = &self.diff_view {
                    let diff_view = diff_view_cell.borrow();
                    if let Some(hunk_index) = diff_view.get_current_hunk_index() {
                        if hunk_index < diff.hunks.len() {
                            (
                                Some((hunk_index, diff.hunks[hunk_index].clone())),
                                diff_state.file_path.clone(),
                            )
                        } else {
                            (None, String::new())
                        }
                    } else {
                        (None, String::new())
                    }
                } else {
                    (None, String::new())
                }
            } else {
                (None, String::new())
            }
        };

        if let Some((_hunk_index, hunk)) = hunk_info {
            let hunk_stager = HunkStager::new(&self.repository);

            match hunk_stager.stage_hunk(&file_path, &hunk) {
                Ok(result) => {
                    self.feedback_manager.show_result(result);
                    // Refresh both status and diff view
                    self.refresh_status_and_diff();
                }
                Err(err) => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(format!(
                            "Failed to stage hunk: {}",
                            err
                        )));
                }
            }
        }
    }

    fn unstage_current_hunk(&mut self) {
        // Extract the required data first to avoid borrowing issues
        let (hunk_info, file_path) = {
            if let (Some(diff), ViewType::Diff(diff_state)) =
                (&self.current_diff, &self.current_view)
            {
                if let Some(diff_view_cell) = &self.diff_view {
                    let diff_view = diff_view_cell.borrow();
                    if let Some(hunk_index) = diff_view.get_current_hunk_index() {
                        if hunk_index < diff.hunks.len() {
                            (
                                Some((hunk_index, diff.hunks[hunk_index].clone())),
                                diff_state.file_path.clone(),
                            )
                        } else {
                            (None, String::new())
                        }
                    } else {
                        (None, String::new())
                    }
                } else {
                    (None, String::new())
                }
            } else {
                (None, String::new())
            }
        };

        if let Some((_hunk_index, hunk)) = hunk_info {
            let hunk_stager = HunkStager::new(&self.repository);

            match hunk_stager.unstage_hunk(&file_path, &hunk) {
                Ok(result) => {
                    self.feedback_manager.show_result(result);
                    // Refresh both status and diff view
                    self.refresh_status_and_diff();
                }
                Err(err) => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(format!(
                            "Failed to unstage hunk: {}",
                            err
                        )));
                }
            }
        }
    }

    fn refresh_status_and_diff(&mut self) {
        // First refresh the repository status
        if self.status.reload(&self.repository).is_ok() {
            self.navigation.update_status(&self.status);
        }

        // Then refresh the diff view if we're currently viewing a diff
        if let ViewType::Diff(diff_state) = &self.current_view.clone() {
            let diff_generator = DiffGenerator::new(self.repository.git2_repo());
            match diff_generator
                .generate_diff(&diff_state.file_path, diff_state.diff_context.clone())
            {
                Ok(new_diff) => {
                    // Preserve navigation state
                    let current_hunk_index = self.diff_view.as_ref().and_then(|cell| {
                        let diff_view = cell.borrow();
                        diff_view.get_current_hunk_index()
                    });

                    // Update the diff and diff view
                    self.current_diff = Some(new_diff.clone());
                    let mut new_diff_view = diff_view::DiffView::new(new_diff);

                    // Try to maintain the current hunk selection
                    if let Some(hunk_index) = current_hunk_index
                        && hunk_index < new_diff_view.get_hunk_count()
                    {
                        // Navigate to the same hunk index if it still exists
                        for _ in 0..hunk_index {
                            new_diff_view.navigate_to_next_hunk();
                        }
                    }

                    self.diff_view = Some(RefCell::new(new_diff_view));
                }
                Err(err) => {
                    // Handle errors by showing error in diff view or feedback
                    match &err {
                        crate::diff::generator::DiffError::BinaryFile(_)
                        | crate::diff::generator::DiffError::FileTooLarge(_, _)
                        | crate::diff::generator::DiffError::TerminalCompatibility(_) => {
                            self.diff_view =
                                Some(RefCell::new(diff_view::DiffView::new_with_error(
                                    diff_state.file_path.clone(),
                                    err,
                                )));
                        }
                        _ => {
                            self.feedback_manager.show_result(
                                crate::operations::OperationResult::new(format!(
                                    "Failed to refresh diff for {}: {}",
                                    diff_state.file_path, err
                                )),
                            );
                        }
                    }
                }
            }
        }
    }
}
