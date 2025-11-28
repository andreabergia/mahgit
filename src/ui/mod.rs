pub mod console;
pub mod diff_renderer;
pub mod diff_view;
pub mod feedback;
pub mod inline_diff;
pub mod input;
pub mod modals;
pub mod navigation;
pub mod status_view;

use crate::config::Config;
use crate::diff::DiffGenerator;
use crate::operations::HunkStager;
use crate::operations::StagingOperations;
use crate::operations::commit::CommitPreparation;
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
use modals::{BoxedModal, ModalContext};
use navigation::{FileDiffKey, NavigationState, SelectedFile, SelectionCursor};
use ratatui::{Terminal, backend::CrosstermBackend};
use status_view::StatusView;
use std::io::{Stdout, stdout};
use std::sync::Arc;

pub struct App {
    should_quit: bool,
    repository: Repository,
    status: RepositoryStatus,
    navigation: NavigationState,
    input_handler: InputHandler,
    feedback_manager: FeedbackManager,
    show_help: bool,
    pending_editor_file: Option<String>,
    config: Arc<Config>,
    /// Currently active modal (if any)
    active_modal: Option<BoxedModal>,
    /// Modal context for tracking what modal system is active
    modal_context: ModalContext,
    /// Pending operation (set when showing modal that requires confirmation or additional input)
    pending_operation: Option<PendingOperation>,
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

#[derive(Clone)]
enum PendingDiscard {
    File { path: String },
    Hunk { path: String, hunk_index: usize },
}

enum PendingOperation {
    Commit(CommitPreparation),
    Discard(PendingDiscard),
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
            config: Arc::new(config),
            active_modal: None,
            modal_context: ModalContext::None,
            pending_operation: None,
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
        use crate::ui::input::InputResult;
        use crossterm::event::{KeyCode, KeyModifiers};

        // If modal is active, route keys to modal first
        if self.modal_context != ModalContext::None {
            match key_event.code {
                KeyCode::Esc => self.cancel_modal(),
                KeyCode::Char(c) if key_event.modifiers == KeyModifiers::NONE => {
                    // Try to handle the key in the modal
                    if !self.handle_modal_input(c) {
                        // Key not handled by modal (e.g., 'n' for cancel)
                        // Close the modal without executing any command
                        self.cancel_modal();
                    }
                }
                _ => {} // Ignore other keys in modal mode
            }
            return;
        }

        let result = self.input_handler.handle_key(key_event);
        match result {
            InputResult::Command(command) => self.handle_command(command),
            InputResult::ShowModal(prefix) => {
                // Future: support other modals (branch, push, pull, log, etc.)
                if prefix == 'c' {
                    self.show_commit_modal();
                }
            }
            InputResult::Pending => {
                // Waiting for second key, do nothing
            }
            InputResult::None => {
                // No action needed
            }
        }
    }

    pub fn process_mouse_event(&mut self, mouse_event: crossterm::event::MouseEvent) {
        // Ignore mouse events when modal is active
        if self.modal_context == ModalContext::None {
            let command = self.input_handler.handle_mouse(mouse_event);
            self.handle_command(command);
        }
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
                                // Check if this was a commit operation
                                if let Some(PendingOperation::Commit(preparation)) =
                                    self.pending_operation.take()
                                {
                                    use std::path::PathBuf;
                                    let temp_file = PathBuf::from(&file_path);

                                    // Read the commit message from the file
                                    match crate::operations::commit::CommitPreparation::read_message_from_file(&temp_file) {
                                        Ok(message) => {
                                            // Execute the commit
                                            use crate::operations::commit::CommitOperations;
                                            let commit_ops = CommitOperations::new(&self.repository);

                                            match commit_ops.execute_commit(&message, &preparation.flags) {
                                                Ok(oid) => {
                                                    self.feedback_manager.show_result(
                                                        crate::operations::OperationResult::new(format!(
                                                            "Created commit: {}",
                                                            oid
                                                        )),
                                                    );
                                                    self.refresh_status_after_operation();
                                                }
                                                Err(err) => {
                                                    self.feedback_manager.show_result(
                                                        crate::operations::OperationResult::new(format!(
                                                            "Failed to create commit: {}",
                                                            err
                                                        )),
                                                    );
                                                }
                                            }
                                        }
                                        Err(err) => {
                                            self.feedback_manager.show_result(
                                                crate::operations::OperationResult::new(format!(
                                                    "Failed to read commit message: {}",
                                                    err
                                                )),
                                            );
                                        }
                                    }

                                    // Clean up the temp file
                                    let _ = std::fs::remove_file(&temp_file);
                                } else {
                                    // Regular file edit - refresh status
                                    self.refresh_status();
                                }
                            } else {
                                // Editor exited with error - clean up commit state if any
                                if matches!(
                                    self.pending_operation,
                                    Some(PendingOperation::Commit(_))
                                ) {
                                    self.pending_operation = None;
                                    self.feedback_manager.show_result(
                                        crate::operations::OperationResult::new(
                                            "Commit aborted: editor exited with error".to_string(),
                                        ),
                                    );
                                } else {
                                    self.feedback_manager.show_result(
                                        crate::operations::OperationResult::new(
                                            "Editor exited with error".to_string(),
                                        ),
                                    );
                                }
                            }
                        }
                        Err(err) => {
                            // Clean up commit state if any
                            if matches!(self.pending_operation, Some(PendingOperation::Commit(_))) {
                                self.pending_operation = None;
                            }
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

            // Check if we should show a modal (timeout reached for prefix key)
            if let Some(prefix) = self.input_handler.should_show_modal() {
                // Future: support other modals (branch, push, pull, log, etc.)
                if prefix == 'c' {
                    self.show_commit_modal();
                }
                self.input_handler.clear_prefix_state();
            }

            // Calculate poll timeout: use remaining time until modal or 16ms, whichever is shorter
            let poll_timeout = if let Some(remaining) = self.input_handler.time_until_modal() {
                remaining.min(std::time::Duration::from_millis(16))
            } else {
                std::time::Duration::from_millis(16)
            };

            if event::poll(poll_timeout)? {
                match event::read()? {
                    Event::Key(key) => {
                        self.process_key_event(key);
                    }
                    Event::Mouse(mouse) => {
                        // Ignore mouse events when modal is active
                        if self.modal_context == ModalContext::None {
                            let command = self.input_handler.handle_mouse(mouse);
                            self.handle_command(command);
                        }
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
            Command::CloseHelp => {
                if self.show_help {
                    self.show_help = false;
                } else {
                    self.should_quit = true;
                }
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
            Command::DiscardFile => {
                self.discard_selected_file();
            }
            Command::ConfirmDiscard => {
                self.execute_confirmed_discard();
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
            Command::IncreaseHunkContext => self.increase_hunk_context(),
            Command::DecreaseHunkContext => self.decrease_hunk_context(),
            Command::PageForward => self.scroll_full_page(VerticalDirection::Down),
            Command::MoveUpHierarchy => self.move_up_hierarchy(),
            Command::MoveDownHierarchy => self.move_down_hierarchy(),
            Command::GoToTopOfDiff => {
                // TODO: Implement inline diff navigation
            }
            Command::GoToBottomOfDiff => {
                // TODO: Implement inline diff navigation
            }
            Command::OpenCommitModal => {
                self.show_commit_modal();
            }
            Command::Commit(mode) => {
                self.handle_commit_command(mode);
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

    fn discard_selected_file(&mut self) {
        // Check if we're operating on a hunk
        if let Some(selected_file) = self.navigation.get_selected_file(&self.status) {
            // Check context - only allow for unstaged, untracked, and conflicted files
            use navigation::FileContext;
            match selected_file.context {
                FileContext::Staged => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(
                            "Cannot discard staged file. Unstage first with 'u'.".to_string(),
                        ));
                }
                FileContext::Unstaged | FileContext::Conflicted => {
                    // Check if operating on hunk
                    if self.inline_hunk_applicable_for_discard() {
                        self.show_discard_hunk_confirmation();
                        return;
                    }
                    // Otherwise, show confirmation for the whole file
                    self.show_discard_file_confirmation(&selected_file);
                }
                FileContext::Untracked => {
                    // For untracked files, show confirmation
                    self.show_discard_file_confirmation(&selected_file);
                }
            }
        }
    }

    fn show_discard_file_confirmation(&mut self, selected_file: &SelectedFile) {
        use navigation::FileContext;
        let is_tracked = !matches!(selected_file.context, FileContext::Untracked);

        let message = if is_tracked {
            format!("Discard all changes to '{}'?", selected_file.path)
        } else {
            format!(
                "Delete untracked file '{}'? (Cannot be undone)",
                selected_file.path
            )
        };

        self.pending_operation = Some(PendingOperation::Discard(PendingDiscard::File {
            path: selected_file.path.clone(),
        }));

        let modal = modals::ConfirmModal::new(message, Command::ConfirmDiscard);
        self.active_modal = Some(Box::new(modal));
        self.modal_context = ModalContext::Confirm;
    }

    fn show_discard_hunk_confirmation(&mut self) {
        if let Some(selected_file) = self.navigation.get_selected_file(&self.status) {
            let diff_key = FileDiffKey::new(selected_file.path.clone(), selected_file.context);
            if let Some(state) = self.navigation.get_file_diff(&diff_key) {
                let current_hunk = match self.navigation.current_cursor() {
                    Some(SelectionCursor::Hunk { hunk_index, .. }) => hunk_index,
                    _ => state.current_hunk,
                };

                if let Some(diff) = &state.diff
                    && current_hunk < diff.hunks.len()
                {
                    let hunk = &diff.hunks[current_hunk];
                    let message = format!(
                        "Discard hunk in '{}' (lines {}-{})?",
                        selected_file.path,
                        hunk.header.old_start,
                        hunk.header.old_start + hunk.header.old_lines
                    );

                    self.pending_operation =
                        Some(PendingOperation::Discard(PendingDiscard::Hunk {
                            path: selected_file.path.clone(),
                            hunk_index: current_hunk,
                        }));

                    let modal = modals::ConfirmModal::new(message, Command::ConfirmDiscard);
                    self.active_modal = Some(Box::new(modal));
                    self.modal_context = ModalContext::Confirm;
                }
            }
        }
    }

    fn execute_confirmed_discard(&mut self) {
        let Some(PendingOperation::Discard(pending)) = self.pending_operation.take() else {
            return;
        };

        match pending {
            PendingDiscard::File { path, .. } => {
                let discard_ops = crate::operations::DiscardOperations::new(&self.repository);
                match discard_ops.discard_file(&path) {
                    Ok(result) => {
                        self.feedback_manager.show_result(result);
                        self.refresh_status_after_operation();
                    }
                    Err(err) => {
                        self.feedback_manager
                            .show_result(crate::operations::OperationResult::new(format!(
                                "Failed to discard {}: {}",
                                path, err
                            )));
                    }
                }
            }
            PendingDiscard::Hunk { path, hunk_index } => {
                self.execute_discard_hunk(&path, hunk_index);
            }
        }
    }

    fn execute_discard_hunk(&mut self, path: &str, hunk_index: usize) {
        if let Some(selected_file) = self.navigation.get_selected_file(&self.status) {
            let diff_key = FileDiffKey::new(selected_file.path.clone(), selected_file.context);
            if let Some(state) = self.navigation.get_file_diff(&diff_key)
                && let Some(diff) = &state.diff
                && let Some(hunk) = diff.hunks.get(hunk_index)
            {
                let discarder = crate::operations::HunkDiscarder::new(&self.repository);
                match discarder.discard_hunk(path, hunk) {
                    Ok(result) => {
                        let prev_ctx = state.diff_context.clone();
                        self.feedback_manager.show_result(result);
                        self.refresh_status_after_operation();

                        // Refresh the diff
                        let diff_generator =
                            crate::diff::DiffGenerator::new(self.repository.git2_repo());
                        if let Ok(new_diff) =
                            diff_generator.generate_diff(&diff_key.path, prev_ctx.clone())
                        {
                            self.navigation
                                .set_file_diff(diff_key.clone(), new_diff, prev_ctx);

                            // Update cursor position
                            if let Some((section, file_index, _)) =
                                self.navigation.cursor_position()
                                && let Some(state2) = self.navigation.get_file_diff(&diff_key)
                            {
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
                                        hunk_index: hunk_index.min(len.saturating_sub(1)),
                                    }
                                };

                                self.navigation
                                    .apply_cursor(&self.status, Some(next_cursor));
                            }
                        }
                    }
                    Err(e) => {
                        self.feedback_manager
                            .show_result(crate::operations::OperationResult::new(format!(
                                "Hunk discard failed: {}",
                                e
                            )));
                    }
                }
            }
        }
    }

    fn inline_hunk_applicable_for_discard(&self) -> bool {
        if !self.is_inline_diff_focused() {
            return false;
        }
        if let Some(selected) = self.navigation.get_selected_file(&self.status) {
            use navigation::FileContext;
            return matches!(
                selected.context,
                FileContext::Unstaged | FileContext::Conflicted
            );
        }
        false
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

        // 1. Always render the status view (now with inline diffs)
        let status_view = StatusView::new(&self.status, &self.navigation, &self.config);
        status_view.render(f, area);

        // 2. Render feedback message if there is one
        if let Some(feedback) = self.feedback_manager.get_current_message() {
            self.render_feedback_message(f, feedback);
        }

        // 3. Render help overlay (if shown)
        if self.show_help {
            self.render_help_overlay(f);
        }

        // 4. Render active modal (if any) - renders on top
        if let Some(modal) = &self.active_modal {
            modal.render(f, area, &self.config);
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
            text::{Line, Span},
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

        // Style the help items based on whether they are headers or keybindings
        let help_items: Vec<ListItem> = help_text
            .into_iter()
            .map(|text| {
                if text.is_empty() {
                    ListItem::new(Line::from(""))
                } else if !text.starts_with("  ") {
                    // This is a header (e.g., "Navigation:")
                    ListItem::new(Line::from(Span::styled(
                        text,
                        Style::default()
                            .fg(self.config.theme.help_key)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    )))
                } else {
                    // This is a keybinding line, split into key and description
                    // Format: "  key     description"
                    if let Some(split_pos) = text.find("     ") {
                        let key_part = &text[..split_pos];
                        let desc_part = &text[split_pos..];
                        ListItem::new(Line::from(vec![
                            Span::styled(key_part, Style::default().fg(self.config.theme.help_key)),
                            Span::styled(
                                desc_part,
                                Style::default().fg(self.config.theme.help_desc),
                            ),
                        ]))
                    } else {
                        ListItem::new(Line::from(Span::styled(
                            text,
                            Style::default().fg(self.config.theme.help_desc),
                        )))
                    }
                }
            })
            .collect();

        let help_block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT) // No bottom border for slide-up effect
            .title(" Help - Press '?' or 'Esc' to close ")
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

    fn increase_hunk_context(&mut self) {
        let Some(sel) = self.navigation.get_selected_file(&self.status) else {
            return;
        };

        let diff_key = FileDiffKey::new(sel.path.clone(), sel.context);

        let (file_path, diff_context, current_context) = {
            let Some(state) = self.navigation.get_file_diff(&diff_key) else {
                return;
            };
            let Some(_diff) = &state.diff else {
                return;
            };
            (
                state.file_path.clone(),
                state.diff_context.clone(),
                state.context_lines,
            )
        };

        let new_context = current_context + 3;

        if let Some(state) = self.navigation.get_file_diff_mut(&diff_key) {
            state.context_lines = new_context;
        }

        self.regenerate_diff_with_context(&diff_key, &file_path, diff_context);
    }

    fn decrease_hunk_context(&mut self) {
        let Some(sel) = self.navigation.get_selected_file(&self.status) else {
            return;
        };

        let diff_key = FileDiffKey::new(sel.path.clone(), sel.context);

        let (file_path, diff_context, current_context) = {
            let Some(state) = self.navigation.get_file_diff(&diff_key) else {
                return;
            };
            let Some(_diff) = &state.diff else {
                return;
            };
            (
                state.file_path.clone(),
                state.diff_context.clone(),
                state.context_lines,
            )
        };

        if current_context > 3 {
            let new_context = current_context.saturating_sub(3);

            if let Some(state) = self.navigation.get_file_diff_mut(&diff_key) {
                state.context_lines = new_context;
            }

            self.regenerate_diff_with_context(&diff_key, &file_path, diff_context);
        }
    }

    fn regenerate_diff_with_context(
        &mut self,
        diff_key: &FileDiffKey,
        file_path: &str,
        diff_context: crate::diff::DiffContext,
    ) {
        let (context_lines, current_hunk) = {
            let state = self.navigation.get_file_diff(diff_key);
            let context_lines = state.map(|s| s.context_lines).unwrap_or(3);
            let current_hunk = state.map(|s| s.current_hunk).unwrap_or(0);
            (context_lines, current_hunk)
        };

        let generator = DiffGenerator::new(self.repository.git2_repo());
        let result = generator.generate_diff_with_context(
            file_path,
            diff_context.clone(),
            Some(context_lines as u32),
        );

        match result {
            Ok(new_diff) => {
                let merged_diff = crate::diff::merge_adjacent_hunks(new_diff);
                self.navigation
                    .set_file_diff(diff_key.clone(), merged_diff, diff_context);

                if let Some(state) = self.navigation.get_file_diff_mut(diff_key) {
                    state.context_lines = context_lines;
                    state.current_hunk = current_hunk;
                }
            }
            Err(err) => {
                self.feedback_manager
                    .show_result(crate::operations::OperationResult::new(format!(
                        "Failed to regenerate diff: {}",
                        err
                    )));
            }
        }
    }

    // Commit operations

    /// Handle a commit command
    fn handle_commit_command(&mut self, mode: input::CommitMode) {
        use crate::operations::commit::CommitOperations;

        let commit_ops = CommitOperations::new(&self.repository);

        // Prepare the commit based on the mode
        let preparation = match commit_ops.prepare_commit(mode) {
            Ok(prep) => prep,
            Err(err) => {
                self.feedback_manager
                    .show_result(crate::operations::OperationResult::new(format!(
                        "Failed to prepare commit: {}",
                        err
                    )));
                return;
            }
        };

        // For extend mode (no_edit), execute immediately without opening editor
        if preparation.flags.no_edit {
            // For extend mode, verify there are changes to commit
            match commit_ops.has_staged_changes() {
                Ok(false) => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(
                            "No staged changes to extend commit with".to_string(),
                        ));
                    return;
                }
                Err(err) => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(format!(
                            "Failed to check for staged changes: {}",
                            err
                        )));
                    return;
                }
                Ok(true) => {
                    // Continue with commit execution
                }
            }

            match commit_ops.execute_commit(&preparation.message_template, &preparation.flags) {
                Ok(oid) => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(format!(
                            "Extended commit: {}",
                            oid
                        )));
                    self.refresh_status_after_operation();
                }
                Err(err) => {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(format!(
                            "Failed to extend commit: {}",
                            err
                        )));
                }
            }
            return;
        }

        // For other modes, open editor with message template
        match preparation.create_message_file() {
            Ok(temp_file) => {
                // Convert PathBuf to String
                if let Some(file_path_str) = temp_file.to_str() {
                    self.pending_editor_file = Some(file_path_str.to_string());
                    self.pending_operation = Some(PendingOperation::Commit(preparation));
                } else {
                    self.feedback_manager
                        .show_result(crate::operations::OperationResult::new(
                            "Failed to create commit message file: invalid path".to_string(),
                        ));
                }
            }
            Err(err) => {
                self.feedback_manager
                    .show_result(crate::operations::OperationResult::new(format!(
                        "Failed to create commit message file: {}",
                        err
                    )));
            }
        }
    }

    // Modal management methods

    /// Show the commit modal
    fn show_commit_modal(&mut self) {
        let modal = modals::CommitModal::new();
        self.active_modal = Some(Box::new(modal));
        self.modal_context = ModalContext::Commit;
    }

    /// Close the currently active modal
    fn close_modal(&mut self) {
        self.active_modal = None;
        self.modal_context = ModalContext::None;
    }

    fn cancel_modal(&mut self) {
        self.close_modal();
        // Clear any pending operations when modal is cancelled
        self.pending_operation = None;
    }

    /// Handle input when a modal is active
    ///
    /// Returns true if the key was handled by the modal
    fn handle_modal_input(&mut self, key: char) -> bool {
        if let Some(modal) = &self.active_modal
            && let Some(cmd) = modal.handle_key(key)
        {
            self.close_modal();
            self.handle_command(cmd);
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::commit::{CommitOperations, CommitPreparation};
    use std::fs;

    fn create_test_config() -> Config {
        Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        }
    }

    fn create_test_repo(_name: &str) -> (Repository, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path();

        let git_repo = git2::Repository::init(repo_path).unwrap();

        // Configure user for commits
        let mut config = git_repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        let repository = Repository::from_git2_repo(git_repo);

        (repository, temp_dir)
    }

    fn create_test_file(repo_dir: &std::path::Path, name: &str, content: &str) {
        let file_path = repo_dir.join(name);
        fs::write(&file_path, content).unwrap();
    }

    #[test]
    fn test_commit_normal_prepare_and_execute() {
        let (repo, temp_dir) = create_test_repo("commit_normal");
        let repo_path = temp_dir.path();

        // Create and stage a file
        create_test_file(repo_path, "test.txt", "test content");
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);

        // Prepare normal commit
        let preparation = commit_ops
            .prepare_commit(input::CommitMode::Normal)
            .unwrap();
        assert_eq!(preparation.mode, input::CommitMode::Normal);
        assert!(!preparation.flags.amend);
        assert!(!preparation.flags.no_edit);

        // Create message file and read it back
        let temp_file = preparation.create_message_file().unwrap();
        fs::write(&temp_file, "Test commit message\n").unwrap();

        let message = CommitPreparation::read_message_from_file(&temp_file).unwrap();
        assert_eq!(message, "Test commit message");

        // Execute commit
        let commit_oid = commit_ops
            .execute_commit(&message, &preparation.flags)
            .unwrap();

        // Verify commit was created
        let git_repo = repo.git2_repo();
        let commit = git_repo.find_commit(commit_oid).unwrap();
        assert_eq!(commit.message().unwrap(), "Test commit message");

        // Clean up
        fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_commit_amend_prepare_and_execute() {
        let (repo, temp_dir) = create_test_repo("commit_amend");
        let repo_path = temp_dir.path();

        // Create initial commit
        create_test_file(repo_path, "test.txt", "initial content");
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);
        let initial_oid = commit_ops
            .execute_commit("Initial commit", &Default::default())
            .unwrap();

        // Make a change and stage it
        create_test_file(repo_path, "test.txt", "modified content");
        repo.add_to_index("test.txt").unwrap();

        // Prepare amend commit
        let preparation = commit_ops.prepare_commit(input::CommitMode::Amend).unwrap();
        assert_eq!(preparation.mode, input::CommitMode::Amend);
        assert!(preparation.flags.amend);
        assert!(!preparation.flags.no_edit);
        assert_eq!(preparation.message_template, "Initial commit");

        // Execute amend with modified message
        let amended_message = "Amended commit message";
        let amended_oid = commit_ops
            .execute_commit(amended_message, &preparation.flags)
            .unwrap();

        // Verify the commit was amended (different OID)
        assert_ne!(initial_oid, amended_oid);

        // Verify the amended commit has the new message
        let git_repo = repo.git2_repo();
        let amended_commit = git_repo.find_commit(amended_oid).unwrap();
        assert_eq!(amended_commit.message().unwrap(), amended_message);

        // Verify HEAD points to the amended commit
        let head = git_repo.head().unwrap();
        let head_commit = head.peel_to_commit().unwrap();
        assert_eq!(head_commit.id(), amended_oid);
    }

    #[test]
    fn test_commit_extend_immediate_execution() {
        let (repo, temp_dir) = create_test_repo("commit_extend");
        let repo_path = temp_dir.path();

        // Create initial commit
        create_test_file(repo_path, "test.txt", "initial content");
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);
        let initial_oid = commit_ops
            .execute_commit("Initial commit", &Default::default())
            .unwrap();

        // Make a change and stage it
        create_test_file(repo_path, "test.txt", "extended content");
        repo.add_to_index("test.txt").unwrap();

        // Prepare extend commit
        let preparation = commit_ops
            .prepare_commit(input::CommitMode::Extend)
            .unwrap();
        assert_eq!(preparation.mode, input::CommitMode::Extend);
        assert!(preparation.flags.amend);
        assert!(preparation.flags.no_edit);

        // Execute extend (should use existing message)
        let extended_oid = commit_ops
            .execute_commit(&preparation.message_template, &preparation.flags)
            .unwrap();

        // Verify the commit was amended with the same message
        assert_ne!(initial_oid, extended_oid);

        let git_repo = repo.git2_repo();
        let extended_commit = git_repo.find_commit(extended_oid).unwrap();
        assert_eq!(extended_commit.message().unwrap(), "Initial commit");
    }

    #[test]
    fn test_commit_reword_prepare() {
        let (repo, temp_dir) = create_test_repo("commit_reword");
        let repo_path = temp_dir.path();

        // Create initial commit
        create_test_file(repo_path, "test.txt", "content");
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);
        commit_ops
            .execute_commit("Initial commit", &Default::default())
            .unwrap();

        // Prepare reword commit (no staged changes needed)
        let preparation = commit_ops
            .prepare_commit(input::CommitMode::Reword)
            .unwrap();
        assert_eq!(preparation.mode, input::CommitMode::Reword);
        assert!(preparation.flags.amend);
        assert!(!preparation.flags.no_edit);
        assert_eq!(preparation.message_template, "Initial commit");
    }

    #[test]
    fn test_commit_with_empty_message_fails() {
        let (repo, temp_dir) = create_test_repo("commit_empty_message");
        let repo_path = temp_dir.path();

        // Create and stage a file
        create_test_file(repo_path, "test.txt", "content");
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);
        let preparation = commit_ops
            .prepare_commit(input::CommitMode::Normal)
            .unwrap();

        // Create message file with only comments
        let temp_file = preparation.create_message_file().unwrap();
        fs::write(&temp_file, "# Only comments\n# More comments\n").unwrap();

        // Reading should fail due to empty message
        let result = CommitPreparation::read_message_from_file(&temp_file);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("empty commit message")
        );

        // Clean up
        fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_commit_message_filtering() {
        let (repo, temp_dir) = create_test_repo("commit_message_filter");
        let repo_path = temp_dir.path();

        create_test_file(repo_path, "test.txt", "content");
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);
        let preparation = commit_ops
            .prepare_commit(input::CommitMode::Normal)
            .unwrap();

        // Create message file with comments and content
        let temp_file = preparation.create_message_file().unwrap();
        fs::write(
            &temp_file,
            "Actual commit message\n\n# This is a comment\nSecond line\n# Another comment\n",
        )
        .unwrap();

        let message = CommitPreparation::read_message_from_file(&temp_file).unwrap();
        assert_eq!(message, "Actual commit message\n\nSecond line");

        // Clean up
        fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_app_handle_commit_command_normal() {
        let (repo, temp_dir) = create_test_repo("app_commit_normal");
        let repo_path = temp_dir.path();

        // Create an initial commit first so RepositoryStatus can work
        create_test_file(repo_path, "initial.txt", "initial");
        repo.add_to_index("initial.txt").unwrap();
        let commit_ops = CommitOperations::new(&repo);
        commit_ops
            .execute_commit("Initial commit", &Default::default())
            .unwrap();

        // Create and stage a file
        create_test_file(repo_path, "test.txt", "content");
        repo.add_to_index("test.txt").unwrap();

        // Reload status to see the staged file
        let status = RepositoryStatus::new(&repo).unwrap();
        let mut app = App::new(repo, status, create_test_config());

        // Handle commit command
        app.handle_commit_command(input::CommitMode::Normal);

        // Verify pending_editor_file was set
        assert!(app.pending_editor_file.is_some());

        // Verify pending_operation was set
        assert!(matches!(
            app.pending_operation,
            Some(PendingOperation::Commit(_))
        ));
        let pending = match &app.pending_operation {
            Some(PendingOperation::Commit(prep)) => prep,
            _ => panic!("Expected pending commit operation"),
        };
        assert_eq!(pending.mode, input::CommitMode::Normal);
        assert!(!pending.flags.no_edit);
    }

    #[test]
    fn test_app_handle_commit_command_extend() {
        let (repo, temp_dir) = create_test_repo("app_commit_extend");
        let repo_path = temp_dir.path();

        // Create initial commit
        create_test_file(repo_path, "test.txt", "initial");
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);
        commit_ops
            .execute_commit("Initial commit", &Default::default())
            .unwrap();

        // Make and stage a change
        create_test_file(repo_path, "test.txt", "extended");
        repo.add_to_index("test.txt").unwrap();

        // Reload status
        let status = RepositoryStatus::new(&repo).unwrap();
        let mut app = App::new(repo, status, create_test_config());

        // Handle extend commit command
        app.handle_commit_command(input::CommitMode::Extend);

        // Verify no editor file was set (extend executes immediately)
        assert!(app.pending_editor_file.is_none());
        assert!(app.pending_operation.is_none());

        // Verify commit was created (check feedback message contains "Extended commit")
        let feedback = app.feedback_manager.get_current_message();
        assert!(feedback.is_some());
        assert!(feedback.unwrap().message.contains("Extended commit"));
    }

    #[test]
    fn test_app_handle_commit_command_extend_no_staged_changes() {
        let (repo, temp_dir) = create_test_repo("app_commit_extend_no_changes");
        let repo_path = temp_dir.path();

        // Create initial commit
        create_test_file(repo_path, "test.txt", "initial");
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);
        commit_ops
            .execute_commit("Initial commit", &Default::default())
            .unwrap();

        // DO NOT stage any changes - this is the key difference
        // Reload status
        let status = RepositoryStatus::new(&repo).unwrap();
        let mut app = App::new(repo, status, create_test_config());

        // Handle extend commit command
        app.handle_commit_command(input::CommitMode::Extend);

        // Verify no editor file was set
        assert!(app.pending_editor_file.is_none());
        assert!(app.pending_operation.is_none());

        // Verify the operation was rejected with appropriate message
        let feedback = app.feedback_manager.get_current_message();
        assert!(feedback.is_some());
        assert!(
            feedback
                .unwrap()
                .message
                .contains("No staged changes to extend commit with")
        );
    }

    #[test]
    fn test_commit_without_head_initial_commit() {
        let (repo, temp_dir) = create_test_repo("initial_commit");
        let repo_path = temp_dir.path();

        // Create and stage a file (no initial commit yet)
        create_test_file(repo_path, "test.txt", "content");
        repo.add_to_index("test.txt").unwrap();

        let commit_ops = CommitOperations::new(&repo);

        // Should be able to prepare normal commit even without HEAD
        let preparation = commit_ops
            .prepare_commit(input::CommitMode::Normal)
            .unwrap();
        assert_eq!(preparation.mode, input::CommitMode::Normal);

        // Execute the initial commit
        let commit_oid = commit_ops
            .execute_commit("Initial commit", &preparation.flags)
            .unwrap();

        // Verify the commit was created
        let git_repo = repo.git2_repo();
        let commit = git_repo.find_commit(commit_oid).unwrap();
        assert_eq!(commit.parent_count(), 0); // Initial commit has no parents
    }

    #[test]
    fn test_amend_without_head_fails() {
        let (repo, _temp_dir) = create_test_repo("amend_no_head");

        let commit_ops = CommitOperations::new(&repo);

        // Attempting to prepare amend without HEAD should fail
        let result = commit_ops.prepare_commit(input::CommitMode::Amend);
        assert!(result.is_err());
    }
}
