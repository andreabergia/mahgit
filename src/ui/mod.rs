pub mod console;
pub mod diff_view;
pub mod feedback;
pub mod input;
pub mod navigation;
pub mod status_view;

use crate::config::Config;
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
use navigation::{FileDiffKey, NavigationState, SelectedFile, SelectionCursor};
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
    pending_editor_file: Option<String>,
    config: Config,
}

#[derive(Clone, PartialEq)]
pub enum ViewType {
    Status, // Remove Diff variant - everything stays in Status view
}

enum StageAction {
    Stage,
    Unstage,
}

enum VerticalDirection {
    Up,
    Down,
}

impl App {
    pub fn new(repository: Repository, status: RepositoryStatus, config: Config) -> Self {
        let navigation = NavigationState::new(&status);
        Self {
            should_quit: false,
            repository,
            status,
            navigation,
            input_handler: InputHandler::new(),
            feedback_manager: FeedbackManager::new(),
            show_help: false,
            pending_editor_file: None,
            config,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
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
            // Handle pending editor file if set
            if let Some(file_path) = self.pending_editor_file.take() {
                // Suspend terminal before opening editor
                if let Err(e) = self.suspend_terminal() {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(format!(
                            "Failed to suspend terminal: {}",
                            e
                        )));
                } else {
                    // Open the file in the editor
                    let editor_result = editor::open_file_in_editor(&self.repository, &file_path);

                    // Restore terminal after editor exits
                    if let Err(e) = self.resume_terminal() {
                        eprintln!("Failed to resume terminal: {}", e);
                        std::process::exit(1);
                    }

                    // Clear the terminal to ensure clean state
                    terminal.clear()?;

                    // Handle the editor result
                    match editor_result {
                        Ok(success) => {
                            if success {
                                // Refresh status after editing in case the file was modified
                                self.refresh_status();
                            } else {
                                self.feedback_manager.show_result(
                                    crate::operations::OperationResult::new(
                                        "Editor exited with error".to_string(),
                                    ),
                                );
                            }
                        }
                        Err(err) => {
                            self.feedback_manager.show_result(
                                crate::operations::OperationResult::new(format!(
                                    "Failed to open editor: {}",
                                    err
                                )),
                            );
                        }
                    }
                }
            }

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
            Command::MoveUp | Command::ScrollDiffUp => self.move_cursor(VerticalDirection::Up),
            Command::MoveDown | Command::ScrollDiffDown => {
                self.move_cursor(VerticalDirection::Down)
            }
            Command::ScrollViewportUp => self.navigation.scroll_viewport_up(1),
            Command::ScrollViewportDown => self.navigation.scroll_viewport_down(1),
            Command::MoveToTop => self.navigation.move_to_top(&self.status),
            Command::MoveToBottom => self.navigation.move_to_bottom(&self.status),
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
            Command::PageDiffUp => self.scroll_full_page(VerticalDirection::Up),
            Command::PageDiffDown => self.scroll_full_page(VerticalDirection::Down),
            Command::PreviousHunk => self.scroll_half_page(VerticalDirection::Up),
            Command::NextHunk => self.scroll_half_page(VerticalDirection::Down),
            Command::JumpToPreviousHunk => self.jump_within_parent(VerticalDirection::Up),
            Command::JumpToNextHunk => self.jump_within_parent(VerticalDirection::Down),
            Command::PageForward => self.scroll_full_page(VerticalDirection::Down),
            Command::MoveUpHierarchy => self.move_up_hierarchy(),
            Command::MoveDownHierarchy => self.move_down_hierarchy(),
            Command::GoToTopOfDiff => {
                // TODO: Implement inline diff navigation
            }
            Command::GoToBottomOfDiff => {
                // TODO: Implement inline diff navigation
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
            self.navigation.clear_diff_cache();
            self.navigation.ensure_cursor_valid(&self.status);
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
        if self.inline_hunk_applicable(StageAction::Stage) {
            self.inline_stage_current_hunk();
            return;
        }
        self.execute_staging_operation(|ops, path| ops.stage_file(path), "stage");
    }

    fn unstage_selected_file(&mut self) {
        if self.inline_hunk_applicable(StageAction::Unstage) {
            self.inline_stage_current_hunk();
            return;
        }
        self.execute_staging_operation(|ops, path| ops.unstage_file(path), "unstage");
    }

    fn add_selected_file(&mut self) {
        self.execute_staging_operation(|ops, path| ops.add_untracked_file(path), "add");
    }

    fn refresh_status_after_operation(&mut self) {
        if self.status.reload(&self.repository).is_err() {
            // If reload fails, we still want to continue, just won't have updated status
        } else {
            self.navigation.update_status(&self.status);
            // Clear diff cache when status changes
            self.navigation.clear_diff_cache();
            self.navigation.ensure_cursor_valid(&self.status);
        }
    }

    fn toggle_accordion(&mut self) {
        // Context-aware toggle:
        // - When on a file cursor, toggle the entire file diff
        // - When on a hunk cursor, toggle that specific hunk
        if let Some(cursor) = self.navigation.current_cursor() {
            match cursor {
                SelectionCursor::File { .. } => {
                    self.toggle_inline_diff();
                }
                SelectionCursor::Hunk { hunk_index, .. } => {
                    // Toggle the specific hunk
                    if let Some(selected_file) = self.navigation.get_selected_file(&self.status) {
                        let diff_key =
                            FileDiffKey::new(selected_file.path.clone(), selected_file.context);
                        self.navigation.toggle_hunk_collapsed(&diff_key, hunk_index);
                    }
                }
            }
        }
    }

    fn toggle_inline_diff(&mut self) {
        if let Some(selected_file) = self.navigation.get_selected_file(&self.status) {
            let diff_context = self.determine_diff_context(&selected_file);
            let diff_key = FileDiffKey::new(selected_file.path.clone(), selected_file.context);

            if self.navigation.is_file_diff_expanded(&diff_key)
                || self
                    .navigation
                    .has_cached_diff_for(&diff_key, &diff_context)
            {
                self.navigation
                    .toggle_file_diff_expanded(diff_key.clone(), diff_context);
                self.navigation.reset_inline_diff_selection(&diff_key);
            } else {
                let diff_generator = DiffGenerator::new(self.repository.git2_repo());
                match diff_generator.generate_diff(&selected_file.path, diff_context.clone()) {
                    Ok(diff) => {
                        self.navigation
                            .toggle_file_diff_expanded(diff_key.clone(), diff_context.clone());
                        self.navigation
                            .set_file_diff(diff_key.clone(), diff, diff_context);
                        self.navigation.reset_inline_diff_selection(&diff_key);
                    }
                    Err(err) => {
                        self.feedback_manager
                            .show_result(crate::operations::OperationResult::new(format!(
                                "Failed to generate diff for {}: {}",
                                selected_file.path, err
                            )));
                    }
                }
            }
            self.navigation.ensure_cursor_valid(&self.status);
        }
    }

    fn move_cursor(&mut self, direction: VerticalDirection) {
        match direction {
            VerticalDirection::Up => self.navigation.move_to_previous(&self.status),
            VerticalDirection::Down => self.navigation.move_to_next(&self.status),
        }
        self.navigation.ensure_cursor_valid(&self.status);
    }

    fn scroll_half_page(&self, direction: VerticalDirection) {
        let height = self.navigation.viewport_height().max(1);
        let step = (height / 2).max(1);
        match direction {
            VerticalDirection::Up => self.navigation.scroll_viewport_up(step),
            VerticalDirection::Down => self.navigation.scroll_viewport_down(step),
        }
    }

    fn scroll_full_page(&self, direction: VerticalDirection) {
        let height = self.navigation.viewport_height().max(1);
        match direction {
            VerticalDirection::Up => self.navigation.scroll_viewport_up(height),
            VerticalDirection::Down => self.navigation.scroll_viewport_down(height),
        }
    }

    fn jump_within_parent(&mut self, direction: VerticalDirection) {
        if let Some((section, file_index, hunk_index)) = self.navigation.cursor_position() {
            let next_cursor = match hunk_index {
                Some(_) => {
                    let hunk_count =
                        self.navigation
                            .hunk_count_for_file(&self.status, section, file_index);
                    if hunk_count == 0 {
                        SelectionCursor::File {
                            section,
                            file_index,
                        }
                    } else {
                        let target = match direction {
                            VerticalDirection::Up => 0,
                            VerticalDirection::Down => hunk_count.saturating_sub(1),
                        };
                        SelectionCursor::Hunk {
                            section,
                            file_index,
                            hunk_index: target,
                        }
                    }
                }
                None => {
                    let last_index = self
                        .navigation
                        .section_file_count(&self.status, section)
                        .saturating_sub(1);
                    let target = match direction {
                        VerticalDirection::Up => 0,
                        VerticalDirection::Down => last_index,
                    };
                    SelectionCursor::File {
                        section,
                        file_index: target,
                    }
                }
            };

            self.navigation
                .apply_cursor(&self.status, Some(next_cursor));
            self.navigation.clear_manual_scroll();
        }
    }

    fn is_inline_diff_focused(&self) -> bool {
        matches!(
            self.navigation.current_cursor(),
            Some(SelectionCursor::Hunk { .. })
        )
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
        let Some(sel) = self.navigation.get_selected_file(&self.status) else {
            return;
        };
        let Some((section, file_index, _)) = self.navigation.cursor_position() else {
            return;
        };

        let diff_key = FileDiffKey::new(sel.path.clone(), sel.context);
        let Some(state) = self.navigation.get_file_diff(&diff_key) else {
            return;
        };
        let Some(diff) = &state.diff else {
            return;
        };
        if diff.hunks.is_empty() {
            return;
        }

        let manual_scroll_state = if self.navigation.is_manual_scroll_active() {
            Some(self.navigation.scroll_offset())
        } else {
            None
        };

        let current_hunk = match self.navigation.current_cursor() {
            Some(SelectionCursor::Hunk { hunk_index, .. }) => hunk_index,
            _ => state.current_hunk,
        }
        .min(diff.hunks.len().saturating_sub(1));

        if let Some(hunk) = diff.hunks.get(current_hunk) {
            let stager = HunkStager::new(&self.repository);
            let res = match sel.context {
                FileContext::Unstaged | FileContext::Untracked => {
                    stager.stage_hunk(&sel.path, hunk)
                }
                FileContext::Staged => stager.unstage_hunk(&sel.path, hunk),
                FileContext::Conflicted => Err(crate::repository::RepositoryError::Other(
                    "Cannot modify conflicted files".into(),
                )),
            };
            match res {
                Ok(r) => {
                    let prev_ctx = state.diff_context.clone();
                    let diff_key_clone = diff_key.clone();
                    self.feedback_manager.show_result(r);
                    self.refresh_status_after_operation();

                    let diff_generator =
                        crate::diff::DiffGenerator::new(self.repository.git2_repo());
                    if let Ok(new_diff) =
                        diff_generator.generate_diff(&diff_key_clone.path, prev_ctx.clone())
                    {
                        self.navigation
                            .set_file_diff(diff_key_clone.clone(), new_diff, prev_ctx);

                        if let Some(state2) = self.navigation.get_file_diff(&diff_key_clone) {
                            let len = state2.diff.as_ref().map(|d| d.hunks.len()).unwrap_or(0);
                            let next_cursor = if len == 0 {
                                SelectionCursor::File {
                                    section,
                                    file_index,
                                }
                            } else {
                                SelectionCursor::Hunk {
                                    section,
                                    file_index,
                                    hunk_index: current_hunk.min(len.saturating_sub(1)),
                                }
                            };

                            self.navigation
                                .apply_cursor(&self.status, Some(next_cursor));
                        }

                        if let Some(offset) = manual_scroll_state {
                            self.navigation.set_scroll_offset(offset);
                            self.navigation.set_manual_scroll_active(true);
                        }
                    }
                }
                Err(e) => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(format!(
                            "Hunk operation failed: {}",
                            e
                        )));
                }
            }
        }
    }

    fn inline_hunk_applicable(&self, action: StageAction) -> bool {
        if !self.is_inline_diff_focused() {
            return false;
        }
        if let Some(selected) = self.navigation.get_selected_file(&self.status) {
            use navigation::FileContext;
            return matches!(
                (action, selected.context),
                (StageAction::Stage, FileContext::Unstaged)
                    | (StageAction::Stage, FileContext::Untracked)
                    | (StageAction::Unstage, FileContext::Staged)
            );
        }
        false
    }

    pub fn render(&mut self, f: &mut ratatui::Frame) {
        self.navigation.ensure_cursor_valid(&self.status);
        let area = f.area();

        // Always render the status view (now with inline diffs)
        let status_view = StatusView::new(&self.status, &self.navigation, self.config.tab_width);
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

            // Set the pending editor file to be handled in the event loop
            // This ensures we have access to the terminal for proper cleanup
            self.pending_editor_file = Some(file_path_str.to_string());
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

    fn move_up_hierarchy(&mut self) {
        // Left key: Move up in hierarchy
        // - If on a hunk cursor, move to the file cursor
        // - If on a file cursor with expanded diff, collapse it
        if let Some(cursor) = self.navigation.current_cursor() {
            match cursor {
                SelectionCursor::Hunk {
                    section,
                    file_index,
                    ..
                } => {
                    // Move from hunk to file
                    let new_cursor = SelectionCursor::File {
                        section,
                        file_index,
                    };
                    self.navigation.apply_cursor(&self.status, Some(new_cursor));
                    self.navigation.clear_manual_scroll();
                }
                SelectionCursor::File { .. } => {
                    // If file has expanded diff, collapse it
                    if let Some(selected_file) = self.navigation.get_selected_file(&self.status) {
                        let diff_key =
                            FileDiffKey::new(selected_file.path.clone(), selected_file.context);
                        if self.navigation.is_file_diff_expanded(&diff_key) {
                            self.toggle_inline_diff();
                        }
                    }
                }
            }
        }
    }

    fn move_down_hierarchy(&mut self) {
        // Right key: Move down in hierarchy
        // - If on a file cursor with closed diff, expand it and move to first hunk
        // - If on a file cursor with expanded diff, move to first hunk
        // - If on a hunk cursor, stay at hunk (no-op)
        if let Some(cursor) = self.navigation.current_cursor() {
            match cursor {
                SelectionCursor::File {
                    section,
                    file_index,
                } => {
                    if let Some(selected_file) = self.navigation.get_selected_file(&self.status) {
                        let diff_key =
                            FileDiffKey::new(selected_file.path.clone(), selected_file.context);

                        if self.navigation.is_file_diff_expanded(&diff_key) {
                            // Diff is already expanded, move to first hunk
                            let hunk_count = self.navigation.hunk_count_for_file(
                                &self.status,
                                section,
                                file_index,
                            );
                            if hunk_count > 0 {
                                let new_cursor = SelectionCursor::Hunk {
                                    section,
                                    file_index,
                                    hunk_index: 0,
                                };
                                self.navigation.apply_cursor(&self.status, Some(new_cursor));
                                self.navigation.clear_manual_scroll();
                            }
                        } else {
                            // Diff is closed, expand it and move to first hunk
                            self.toggle_inline_diff();
                            // After expanding, move to first hunk if available
                            let hunk_count = self.navigation.hunk_count_for_file(
                                &self.status,
                                section,
                                file_index,
                            );
                            if hunk_count > 0 {
                                let new_cursor = SelectionCursor::Hunk {
                                    section,
                                    file_index,
                                    hunk_index: 0,
                                };
                                self.navigation.apply_cursor(&self.status, Some(new_cursor));
                                self.navigation.clear_manual_scroll();
                            }
                        }
                    }
                }
                SelectionCursor::Hunk { .. } => {
                    // Already at hunk level, do nothing (no-op)
                }
            }
        }
    }

    // TODO: Implement inline hunk staging methods
}
