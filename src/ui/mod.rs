pub mod console;
pub mod diff_view;
pub mod feedback;
pub mod input;
pub mod navigation;
pub mod status_view;

use crate::diff::DiffGenerator;
use crate::operations::HunkStager;
use crate::operations::StagingOperations;
use crate::operations::editor;
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
use std::io::{Stdout, stdout};

pub struct App {
    should_quit: bool,
    repository: Repository,
    status: RepositoryStatus,
    navigation: NavigationState,
    input_handler: InputHandler,
    feedback_manager: FeedbackManager,
    show_help: bool,
}

#[derive(Clone, PartialEq)]
pub enum ViewType {
    Status, // Remove Diff variant - everything stays in Status view
}

impl App {
    pub fn new(repository: Repository, status: RepositoryStatus) -> Self {
        let navigation = NavigationState::new(&status);
        Self {
            should_quit: false,
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

    // Getters for testing - exposed even in non-test builds for integration tests
    pub fn status(&self) -> &RepositoryStatus {
        &self.status
    }

    pub fn navigation(&self) -> &NavigationState {
        &self.navigation
    }

    /// Toggle section collapsed state (for testing purposes)
    pub fn toggle_section_collapsed(&mut self, section: navigation::StatusSection) {
        self.navigation.toggle_section_collapsed(section);
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

    pub fn process_mouse_event(&mut self, mouse_event: crossterm::event::MouseEvent) {
        let command = self.input_handler.handle_mouse(mouse_event);
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

            if event::poll(std::time::Duration::from_millis(16))? {
                match event::read()? {
                    Event::Key(key) => {
                        let command = self.input_handler.handle_key(key);
                        self.handle_command(command);
                    }
                    Event::Mouse(mouse) => {
                        let command = self.input_handler.handle_mouse(mouse);
                        self.handle_command(command);
                    }
                    _ => {}
                }
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
                if self.is_inline_diff_active() {
                    self.inline_prev_hunk();
                } else {
                    self.navigation.move_up();
                }
            }
            Command::MoveDown => {
                if self.is_inline_diff_active() {
                    self.inline_next_hunk();
                } else {
                    self.navigation.move_down();
                }
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
            Command::EnterDiffView => {
                self.toggle_inline_diff();
            }
            Command::ExitDiffView => {
                // No longer needed - inline diffs don't have exit
            }
            Command::OpenInEditor => {
                self.open_file_in_editor();
            }
            Command::ToggleAccordion => {
                self.toggle_accordion();
            }
            Command::ScrollDiffUp => {
                self.inline_prev_hunk();
            }
            Command::ScrollDiffDown => {
                self.inline_next_hunk();
            }
            Command::PageDiffUp => {
                // For inline diff, page up = previous hunk
                self.inline_prev_hunk();
            }
            Command::PageDiffDown => {
                // For inline diff, page down = next hunk
                self.inline_next_hunk();
            }
            Command::JumpToNextHunk => {
                self.inline_next_hunk();
            }
            Command::JumpToPreviousHunk => {
                self.inline_prev_hunk();
            }
            Command::NextHunk => {
                self.inline_next_hunk();
            }
            Command::PreviousHunk => {
                self.inline_prev_hunk();
            }
            Command::GoToTopOfDiff => {
                // TODO: Implement inline diff navigation
            }
            Command::GoToBottomOfDiff => {
                // TODO: Implement inline diff navigation
            }
            Command::StageHunk => {
                self.inline_stage_current_hunk();
            }
            Command::UnstageHunk => {
                self.inline_stage_current_hunk();
            }
            Command::Unknown => {
                // Ignore unknown commands
            }
            Command::None => {
                // No-op for unhandled events (like unsupported mouse events)
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
            // Clear diff cache when status changes
            self.navigation.clear_diff_cache();
        }
    }

    fn toggle_accordion(&mut self) {
        if self.navigation.is_on_section_header() {
            // Toggle section collapsed/expanded
            let current_section = self.navigation.current_section();
            self.navigation.toggle_section_collapsed(current_section);
        } else {
            // Toggle file diff display
            self.toggle_inline_diff();
        }
    }

    fn toggle_inline_diff(&mut self) {
        if let Some(selected_file) = self.navigation.get_selected_file(&self.status) {
            let diff_context = self.determine_diff_context(&selected_file);

            // Check if diff is already expanded
            if self.navigation.is_file_diff_expanded(&selected_file.path) {
                // Collapse the diff
                self.navigation
                    .toggle_file_diff_expanded(selected_file.path, diff_context);
            } else {
                // Generate the diff first before marking as expanded
                let diff_generator = DiffGenerator::new(self.repository.git2_repo());
                match diff_generator.generate_diff(&selected_file.path, diff_context.clone()) {
                    Ok(diff) => {
                        // Only expand if diff generation succeeds
                        self.navigation.toggle_file_diff_expanded(
                            selected_file.path.clone(),
                            diff_context.clone(),
                        );
                        self.navigation
                            .set_file_diff(selected_file.path, diff, diff_context);
                    }
                    Err(err) => {
                        // Show error in feedback - don't expand the diff
                        self.feedback_manager
                            .show_result(crate::operations::OperationResult::new(format!(
                                "Failed to generate diff for {}: {}",
                                selected_file.path, err
                            )));
                    }
                }
            }
        }
    }

    fn is_inline_diff_active(&self) -> bool {
        if let Some(sel) = self.navigation.get_selected_file(&self.status) {
            if let Some(state) = self.navigation.get_file_diff(&sel.path) {
                return state.expanded
                    && state
                        .diff
                        .as_ref()
                        .map(|d| !d.hunks.is_empty())
                        .unwrap_or(false);
            }
        }
        false
    }

    fn inline_next_hunk(&mut self) {
        if let Some(sel) = self.navigation.get_selected_file(&self.status) {
            self.navigation.next_inline_hunk(&sel.path);
        }
    }

    fn inline_prev_hunk(&mut self) {
        if let Some(sel) = self.navigation.get_selected_file(&self.status) {
            self.navigation.prev_inline_hunk(&sel.path);
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

    fn inline_stage_current_hunk(&mut self) {
        use navigation::FileContext;
        if let Some(sel) = self.navigation.get_selected_file(&self.status) {
            if let Some(state) = self.navigation.get_file_diff(&sel.path) {
                if let Some(diff) = &state.diff {
                    let idx = state.current_hunk.min(diff.hunks.len().saturating_sub(1));
                    if let Some(hunk) = diff.hunks.get(idx) {
                        let stager = HunkStager::new(&self.repository);
                        let res = match sel.context {
                            FileContext::Unstaged | FileContext::Untracked => {
                                stager.stage_hunk(&sel.path, hunk)
                            }
                            FileContext::Staged => stager.unstage_hunk(&sel.path, hunk),
                            FileContext::Conflicted => {
                                Err(crate::repository::RepositoryError::Other(
                                    "Cannot modify conflicted files".into(),
                                ))
                            }
                        };
                        match res {
                            Ok(r) => {
                                // Preserve selection and move to next hunk after refresh
                                let file_path = sel.path.clone();
                                let prev_index = state.current_hunk;
                                let prev_ctx = state.diff_context.clone();

                                self.feedback_manager.show_result(r);
                                self.refresh_status_after_operation();

                                // Regenerate and re-expand inline diff for the same file
                                let diff_generator =
                                    crate::diff::DiffGenerator::new(self.repository.git2_repo());
                                if let Ok(new_diff) =
                                    diff_generator.generate_diff(&file_path, prev_ctx.clone())
                                {
                                    self.navigation.set_file_diff(
                                        file_path.clone(),
                                        new_diff,
                                        prev_ctx,
                                    );
                                    // Move selection to next hunk (same index after removal), clamp
                                    if let Some(state2) = self.navigation.get_file_diff(&file_path)
                                    {
                                        let len = state2
                                            .diff
                                            .as_ref()
                                            .map(|d| d.hunks.len())
                                            .unwrap_or(0);
                                        let next = if len == 0 {
                                            0
                                        } else {
                                            prev_index.min(len.saturating_sub(1))
                                        };
                                        self.navigation
                                            .set_current_inline_hunk_index(&file_path, next);
                                    }
                                }
                            }
                            Err(e) => {
                                self.feedback_manager.show_result(
                                    crate::operations::OperationResult::new(format!(
                                        "Hunk operation failed: {}",
                                        e
                                    )),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn render(&mut self, f: &mut ratatui::Frame) {
        let area = f.area();

        // Always render the status view (now with inline diffs)
        let status_view = StatusView::new(&self.status, &self.navigation);
        status_view.render(f, area);

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

    fn open_file_in_editor(&mut self) {
        if let Some(selected_file) = self.navigation.get_selected_file(&self.status) {
            // Get the repository's working directory
            let workdir = match self.repository.git2_repo().workdir() {
                Some(dir) => dir,
                None => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(
                            "Repository has no working directory".to_string(),
                        ));
                    return;
                }
            };

            // Build the full path to the file
            let file_path = workdir.join(&selected_file.path);
            let file_path_str = match file_path.to_str() {
                Some(s) => s,
                None => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(
                            "Invalid file path".to_string(),
                        ));
                    return;
                }
            };

            // Suspend terminal before opening editor
            if let Err(e) = self.suspend_terminal() {
                self.feedback_manager
                    .show_result(crate::operations::OperationResult::new(format!(
                        "Failed to suspend terminal: {}",
                        e
                    )));
                return;
            }

            // Open the file in the editor
            let editor_result = editor::open_file_in_editor(&self.repository, file_path_str);

            // Restore terminal after editor exits
            if let Err(e) = self.resume_terminal() {
                eprintln!("Failed to resume terminal: {}", e);
                std::process::exit(1);
            }

            // Handle the editor result
            match editor_result {
                Ok(success) => {
                    if success {
                        // Refresh status after editing in case the file was modified
                        self.refresh_status();
                    } else {
                        self.feedback_manager
                            .show_result(crate::operations::OperationResult::new(
                                "Editor exited with error".to_string(),
                            ));
                    }
                }
                Err(err) => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(format!(
                            "Failed to open editor: {}",
                            err
                        )));
                }
            }
        }
    }

    fn suspend_terminal(&self) -> Result<(), Box<dyn std::error::Error>> {
        disable_raw_mode()?;
        execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
        Ok(())
    }

    fn resume_terminal(&self) -> Result<(), Box<dyn std::error::Error>> {
        execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        enable_raw_mode()?;
        Ok(())
    }

    // TODO: Implement inline hunk staging methods
}
