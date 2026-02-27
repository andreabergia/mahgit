use crate::diff::Diff;
use crate::log::LogData;
use crate::repository::CommitFileChange;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogCursor {
    Commit {
        index: usize,
    },
    File {
        commit_index: usize,
        file_index: usize,
    },
    Hunk {
        commit_index: usize,
        file_index: usize,
        hunk_index: usize,
    },
}

/// Expansion state for a single commit in the log
#[derive(Debug)]
pub struct CommitExpansion {
    pub expanded: bool,
    pub files: Vec<CommitFileChange>,
    pub file_diffs: HashMap<usize, Diff>,
    pub expanded_files: HashSet<usize>,
}

pub struct LogNavigationState {
    cursor: Option<LogCursor>,
    expansions: HashMap<usize, CommitExpansion>,
    scroll_offset: Cell<usize>,
    viewport_height: Cell<usize>,
    max_scroll_offset: Cell<usize>,
    manual_scroll_active: Cell<bool>,
}

impl Default for LogNavigationState {
    fn default() -> Self {
        Self::new()
    }
}

impl LogNavigationState {
    pub fn new() -> Self {
        Self {
            cursor: Some(LogCursor::Commit { index: 0 }),
            expansions: HashMap::new(),
            scroll_offset: Cell::new(0),
            viewport_height: Cell::new(1),
            max_scroll_offset: Cell::new(0),
            manual_scroll_active: Cell::new(false),
        }
    }

    pub fn current_cursor(&self) -> Option<LogCursor> {
        self.cursor
    }

    pub fn get_expansion(&self, commit_index: usize) -> Option<&CommitExpansion> {
        self.expansions.get(&commit_index)
    }

    pub fn get_expansion_mut(&mut self, commit_index: usize) -> Option<&mut CommitExpansion> {
        self.expansions.get_mut(&commit_index)
    }

    pub fn set_expansion(&mut self, commit_index: usize, expansion: CommitExpansion) {
        self.expansions.insert(commit_index, expansion);
    }

    pub fn is_commit_expanded(&self, commit_index: usize) -> bool {
        self.expansions
            .get(&commit_index)
            .is_some_and(|e| e.expanded)
    }

    pub fn is_file_expanded(&self, commit_index: usize, file_index: usize) -> bool {
        self.expansions
            .get(&commit_index)
            .is_some_and(|e| e.expanded_files.contains(&file_index))
    }

    pub fn move_to_next(&mut self, log_data: &LogData) {
        self.clear_manual_scroll();
        let Some(cursor) = self.cursor else {
            if !log_data.entries.is_empty() {
                self.cursor = Some(LogCursor::Commit { index: 0 });
            }
            return;
        };

        self.cursor = self.next_cursor(log_data, &cursor);
    }

    pub fn move_to_previous(&mut self, log_data: &LogData) {
        self.clear_manual_scroll();
        let Some(cursor) = self.cursor else {
            if !log_data.entries.is_empty() {
                self.cursor = Some(LogCursor::Commit { index: 0 });
            }
            return;
        };

        self.cursor = self.previous_cursor(log_data, &cursor);
    }

    pub fn move_to_top(&mut self, log_data: &LogData) {
        self.clear_manual_scroll();
        if !log_data.entries.is_empty() {
            self.cursor = Some(LogCursor::Commit { index: 0 });
        }
    }

    pub fn move_to_bottom(&mut self, log_data: &LogData) {
        self.clear_manual_scroll();
        if !log_data.entries.is_empty() {
            let last_commit = log_data.entries.len() - 1;
            // Navigate to the last visible item within the last expanded commit
            if let Some(expansion) = self.expansions.get(&last_commit)
                && expansion.expanded
                && !expansion.files.is_empty()
            {
                let last_file = expansion.files.len() - 1;
                if expansion.expanded_files.contains(&last_file)
                    && let Some(diff) = expansion.file_diffs.get(&last_file)
                    && !diff.hunks.is_empty()
                {
                    self.cursor = Some(LogCursor::Hunk {
                        commit_index: last_commit,
                        file_index: last_file,
                        hunk_index: diff.hunks.len() - 1,
                    });
                    return;
                }
                self.cursor = Some(LogCursor::File {
                    commit_index: last_commit,
                    file_index: last_file,
                });
                return;
            }
            self.cursor = Some(LogCursor::Commit { index: last_commit });
        }
    }

    fn next_cursor(&self, log_data: &LogData, cursor: &LogCursor) -> Option<LogCursor> {
        match *cursor {
            LogCursor::Commit { index } => {
                // If expanded, go into the first file
                if let Some(expansion) = self.expansions.get(&index)
                    && expansion.expanded
                    && !expansion.files.is_empty()
                {
                    return Some(LogCursor::File {
                        commit_index: index,
                        file_index: 0,
                    });
                }
                // Otherwise go to next commit
                if index + 1 < log_data.entries.len() {
                    Some(LogCursor::Commit { index: index + 1 })
                } else {
                    Some(*cursor)
                }
            }
            LogCursor::File {
                commit_index,
                file_index,
            } => {
                // If file is expanded, go into first hunk
                if let Some(expansion) = self.expansions.get(&commit_index) {
                    if expansion.expanded_files.contains(&file_index)
                        && let Some(diff) = expansion.file_diffs.get(&file_index)
                        && !diff.hunks.is_empty()
                    {
                        return Some(LogCursor::Hunk {
                            commit_index,
                            file_index,
                            hunk_index: 0,
                        });
                    }
                    // Go to next file in same commit
                    if file_index + 1 < expansion.files.len() {
                        return Some(LogCursor::File {
                            commit_index,
                            file_index: file_index + 1,
                        });
                    }
                }
                // Go to next commit
                if commit_index + 1 < log_data.entries.len() {
                    Some(LogCursor::Commit {
                        index: commit_index + 1,
                    })
                } else {
                    Some(*cursor)
                }
            }
            LogCursor::Hunk {
                commit_index,
                file_index,
                hunk_index,
            } => {
                if let Some(expansion) = self.expansions.get(&commit_index) {
                    // Next hunk in same file
                    if let Some(diff) = expansion.file_diffs.get(&file_index)
                        && hunk_index + 1 < diff.hunks.len()
                    {
                        return Some(LogCursor::Hunk {
                            commit_index,
                            file_index,
                            hunk_index: hunk_index + 1,
                        });
                    }
                    // Next file in same commit
                    if file_index + 1 < expansion.files.len() {
                        return Some(LogCursor::File {
                            commit_index,
                            file_index: file_index + 1,
                        });
                    }
                }
                // Next commit
                if commit_index + 1 < log_data.entries.len() {
                    Some(LogCursor::Commit {
                        index: commit_index + 1,
                    })
                } else {
                    Some(*cursor)
                }
            }
        }
    }

    fn previous_cursor(&self, log_data: &LogData, cursor: &LogCursor) -> Option<LogCursor> {
        match *cursor {
            LogCursor::Commit { index } => {
                if index == 0 {
                    return Some(*cursor);
                }
                let prev_commit = index - 1;
                // Go to last visible item of previous commit
                self.last_item_of_commit(log_data, prev_commit)
            }
            LogCursor::File {
                commit_index,
                file_index,
            } => {
                if file_index == 0 {
                    // Go back to the commit line
                    return Some(LogCursor::Commit {
                        index: commit_index,
                    });
                }
                // Go to last item of previous file
                let prev_file = file_index - 1;
                if let Some(expansion) = self.expansions.get(&commit_index)
                    && expansion.expanded_files.contains(&prev_file)
                    && let Some(diff) = expansion.file_diffs.get(&prev_file)
                    && !diff.hunks.is_empty()
                {
                    return Some(LogCursor::Hunk {
                        commit_index,
                        file_index: prev_file,
                        hunk_index: diff.hunks.len() - 1,
                    });
                }
                Some(LogCursor::File {
                    commit_index,
                    file_index: prev_file,
                })
            }
            LogCursor::Hunk {
                commit_index,
                file_index,
                hunk_index,
            } => {
                if hunk_index > 0 {
                    Some(LogCursor::Hunk {
                        commit_index,
                        file_index,
                        hunk_index: hunk_index - 1,
                    })
                } else {
                    // Go back to file
                    Some(LogCursor::File {
                        commit_index,
                        file_index,
                    })
                }
            }
        }
    }

    /// Get the last visible item of a commit (for backwards navigation).
    fn last_item_of_commit(&self, _log_data: &LogData, commit_index: usize) -> Option<LogCursor> {
        if let Some(expansion) = self.expansions.get(&commit_index)
            && expansion.expanded
            && !expansion.files.is_empty()
        {
            let last_file = expansion.files.len() - 1;
            if expansion.expanded_files.contains(&last_file)
                && let Some(diff) = expansion.file_diffs.get(&last_file)
                && !diff.hunks.is_empty()
            {
                return Some(LogCursor::Hunk {
                    commit_index,
                    file_index: last_file,
                    hunk_index: diff.hunks.len() - 1,
                });
            }
            return Some(LogCursor::File {
                commit_index,
                file_index: last_file,
            });
        }
        Some(LogCursor::Commit {
            index: commit_index,
        })
    }

    /// Move cursor up in the hierarchy (hunk -> file -> commit).
    pub fn move_up_hierarchy(&mut self) {
        let Some(cursor) = self.cursor else {
            return;
        };
        match cursor {
            LogCursor::Hunk {
                commit_index,
                file_index,
                ..
            } => {
                self.cursor = Some(LogCursor::File {
                    commit_index,
                    file_index,
                });
            }
            LogCursor::File { commit_index, .. } => {
                self.cursor = Some(LogCursor::Commit {
                    index: commit_index,
                });
            }
            LogCursor::Commit { .. } => {
                // Already at top level, do nothing
            }
        }
    }

    // Scroll methods
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset.get()
    }

    pub fn scroll_viewport_up(&self, lines: usize) {
        let current = self.scroll_offset.get();
        self.scroll_offset.set(current.saturating_sub(lines));
        self.manual_scroll_active.set(true);
    }

    pub fn scroll_viewport_down(&self, lines: usize) {
        let current = self.scroll_offset.get();
        let max = self.max_scroll_offset.get();
        self.scroll_offset.set((current + lines).min(max));
        self.manual_scroll_active.set(true);
    }

    pub fn is_manual_scroll_active(&self) -> bool {
        self.manual_scroll_active.get()
    }

    fn clear_manual_scroll(&self) {
        self.manual_scroll_active.set(false);
    }

    pub fn update_viewport_metrics(&self, viewport_height: usize, total_items: usize) {
        self.viewport_height.set(viewport_height);
        self.max_scroll_offset
            .set(total_items.saturating_sub(viewport_height));
    }

    pub fn viewport_height(&self) -> usize {
        self.viewport_height.get()
    }

    pub fn ensure_cursor_valid(&mut self, log_data: &LogData) {
        if log_data.entries.is_empty() {
            self.cursor = None;
            return;
        }
        match self.cursor {
            Some(LogCursor::Commit { index }) if index >= log_data.entries.len() => {
                self.cursor = Some(LogCursor::Commit {
                    index: log_data.entries.len() - 1,
                });
            }
            None => {
                self.cursor = Some(LogCursor::Commit { index: 0 });
            }
            _ => {}
        }
    }

    /// Compute the flat list index of the current cursor for scroll tracking.
    pub fn cursor_flat_index(&self, log_data: &LogData) -> Option<usize> {
        let cursor = self.cursor?;
        let mut flat_index = 0;

        for (commit_idx, _entry) in log_data.entries.iter().enumerate() {
            if matches!(cursor, LogCursor::Commit { index } if index == commit_idx) {
                return Some(flat_index);
            }
            flat_index += 1;

            if let Some(expansion) = self.expansions.get(&commit_idx)
                && expansion.expanded
            {
                for (file_idx, _file) in expansion.files.iter().enumerate() {
                    if matches!(cursor, LogCursor::File { commit_index, file_index }
                            if commit_index == commit_idx && file_index == file_idx)
                    {
                        return Some(flat_index);
                    }
                    flat_index += 1;

                    if expansion.expanded_files.contains(&file_idx)
                        && let Some(diff) = expansion.file_diffs.get(&file_idx)
                    {
                        for (hunk_idx, hunk) in diff.hunks.iter().enumerate() {
                            if matches!(cursor, LogCursor::Hunk { commit_index, file_index, hunk_index }
                                        if commit_index == commit_idx && file_index == file_idx && hunk_index == hunk_idx)
                            {
                                return Some(flat_index);
                            }
                            flat_index += 1; // hunk header
                            flat_index += hunk.lines.len(); // diff lines
                        }
                    }
                }
            }
        }

        None
    }

    pub fn page_up(&mut self, log_data: &LogData) {
        let page_size = self.viewport_height.get().saturating_sub(2).max(1);
        for _ in 0..page_size {
            let prev = self.cursor;
            self.move_to_previous(log_data);
            if self.cursor == prev {
                break;
            }
        }
    }

    pub fn page_down(&mut self, log_data: &LogData) {
        let page_size = self.viewport_height.get().saturating_sub(2).max(1);
        for _ in 0..page_size {
            let prev = self.cursor;
            self.move_to_next(log_data);
            if self.cursor == prev {
                break;
            }
        }
    }
}
