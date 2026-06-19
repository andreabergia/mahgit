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
    pub collapsed_hunks: HashMap<usize, HashSet<usize>>,
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

    pub fn collapse_file(&mut self, commit_index: usize, file_index: usize) {
        self.clear_manual_scroll();

        if let Some(expansion) = self.expansions.get_mut(&commit_index) {
            expansion.expanded_files.remove(&file_index);
        }

        if matches!(
            self.cursor,
            Some(LogCursor::Hunk {
                commit_index: cursor_commit,
                file_index: cursor_file,
                ..
            }) if cursor_commit == commit_index && cursor_file == file_index
        ) {
            self.cursor = Some(LogCursor::File {
                commit_index,
                file_index,
            });
        }
    }

    pub fn toggle_hunk_collapsed(
        &mut self,
        commit_index: usize,
        file_index: usize,
        hunk_index: usize,
    ) {
        self.clear_manual_scroll();

        let Some(expansion) = self.expansions.get_mut(&commit_index) else {
            return;
        };

        let collapsed_hunks = expansion.collapsed_hunks.entry(file_index).or_default();
        if collapsed_hunks.contains(&hunk_index) {
            collapsed_hunks.remove(&hunk_index);
        } else {
            collapsed_hunks.insert(hunk_index);
        }
    }

    pub fn is_hunk_collapsed(
        &self,
        commit_index: usize,
        file_index: usize,
        hunk_index: usize,
    ) -> bool {
        self.expansions
            .get(&commit_index)
            .and_then(|expansion| expansion.collapsed_hunks.get(&file_index))
            .is_some_and(|hunks| hunks.contains(&hunk_index))
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

    pub fn clear_manual_scroll(&self) {
        self.manual_scroll_active.set(false);
    }

    pub fn update_viewport_metrics(&self, viewport_height: usize, total_items: usize) {
        self.viewport_height.set(viewport_height);
        let max = total_items.saturating_sub(viewport_height);
        self.max_scroll_offset.set(max);
        // Re-clamp the offset so a shrunk content area can't leave us scrolled
        // past the end (which would show a blank viewport).
        if self.scroll_offset.get() > max {
            self.scroll_offset.set(max);
        }
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

        for (commit_idx, entry) in log_data.entries.iter().enumerate() {
            if matches!(cursor, LogCursor::Commit { index } if index == commit_idx) {
                return Some(flat_index);
            }
            flat_index += 1;

            if let Some(expansion) = self.expansions.get(&commit_idx)
                && expansion.expanded
            {
                // Metadata block rendered between the commit row and its files.
                flat_index += crate::ui::log_view::commit_metadata_line_count(entry);

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
                            if !self.is_hunk_collapsed(commit_idx, file_idx, hunk_idx) {
                                flat_index += hunk.lines.len(); // diff lines
                            }
                            if diff.hunks.len() > 1 {
                                flat_index += 1; // separator line
                            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{Diff, DiffContext, DiffHunk, DiffLine, HunkHeader, LineRange, LineType};
    use crate::log::{LogData, LogEntry};
    use crate::repository::{CommitChangeType, CommitFileChange};

    fn sample_hunk(label: &str, line_count: usize) -> DiffHunk {
        let lines = (0..line_count)
            .map(|index| DiffLine {
                content: format!("line {index}"),
                line_type: LineType::Context,
                old_line_no: Some(index + 1),
                new_line_no: Some(index + 1),
                inline_diff: None,
            })
            .collect();

        DiffHunk {
            header: HunkHeader {
                raw: format!("@@ {label} @@"),
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
            },
            lines,
            old_range: LineRange { start: 1, count: 1 },
            new_range: LineRange { start: 1, count: 1 },
            stageable: false,
            context_lines: 0,
        }
    }

    fn sample_diff(hunks: Vec<DiffHunk>) -> Diff {
        Diff {
            file_path: "file.txt".to_string(),
            context: DiffContext::WorkingTreeToHead,
            hunks,
            binary: false,
        }
    }

    fn sample_expansion(diff: Diff) -> CommitExpansion {
        let mut file_diffs = HashMap::new();
        file_diffs.insert(0, diff);

        let mut expanded_files = HashSet::new();
        expanded_files.insert(0);

        CommitExpansion {
            expanded: true,
            files: vec![CommitFileChange {
                path: "file.txt".to_string(),
                old_path: None,
                change_type: CommitChangeType::Modified,
            }],
            file_diffs,
            expanded_files,
            collapsed_hunks: HashMap::new(),
        }
    }

    #[test]
    fn collapse_file_from_hunk_cursor_collapses_parent_and_selects_file() {
        let mut navigation = LogNavigationState::new();
        navigation.set_expansion(0, sample_expansion(sample_diff(vec![sample_hunk("h1", 0)])));
        navigation.cursor = Some(LogCursor::Hunk {
            commit_index: 0,
            file_index: 0,
            hunk_index: 0,
        });

        navigation.collapse_file(0, 0);

        assert!(!navigation.is_file_expanded(0, 0));
        assert_eq!(
            navigation.current_cursor(),
            Some(LogCursor::File {
                commit_index: 0,
                file_index: 0
            })
        );
    }

    #[test]
    fn toggle_hunk_collapsed_keeps_file_expanded_and_cursor_on_hunk() {
        let mut navigation = LogNavigationState::new();
        navigation.set_expansion(0, sample_expansion(sample_diff(vec![sample_hunk("h1", 0)])));
        navigation.cursor = Some(LogCursor::Hunk {
            commit_index: 0,
            file_index: 0,
            hunk_index: 0,
        });

        navigation.toggle_hunk_collapsed(0, 0, 0);

        assert!(navigation.is_file_expanded(0, 0));
        assert!(navigation.is_hunk_collapsed(0, 0, 0));
        assert_eq!(
            navigation.current_cursor(),
            Some(LogCursor::Hunk {
                commit_index: 0,
                file_index: 0,
                hunk_index: 0
            })
        );

        navigation.toggle_hunk_collapsed(0, 0, 0);

        assert!(!navigation.is_hunk_collapsed(0, 0, 0));
    }

    #[test]
    fn update_viewport_metrics_reclamps_scroll_offset_when_content_shrinks() {
        let navigation = LogNavigationState::new();

        // Large content: scroll all the way down.
        navigation.update_viewport_metrics(10, 100);
        navigation.scroll_viewport_down(90);
        assert_eq!(navigation.scroll_offset(), 90);

        // Content shrinks; offset must be re-clamped to the new max.
        navigation.update_viewport_metrics(10, 20);
        assert_eq!(navigation.scroll_offset(), 10);
    }

    #[test]
    fn cursor_flat_index_counts_collapsed_hunk_as_header_only() {
        let mut log_data = LogData::new("main".to_string());
        let entry = LogEntry {
            oid: git2::Oid::zero(),
            short_hash: "0000000".to_string(),
            summary: "commit".to_string(),
            message: "commit".to_string(),
            author_name: "Test User".to_string(),
            author_email: "test@example.com".to_string(),
            time: git2::Time::new(0, 0),
            committer_name: "Test User".to_string(),
            committer_email: "test@example.com".to_string(),
            committer_time: git2::Time::new(0, 0),
        };
        let metadata_lines = crate::ui::log_view::commit_metadata_line_count(&entry);
        log_data.entries.push(entry);

        let mut navigation = LogNavigationState::new();
        navigation.set_expansion(
            0,
            sample_expansion(sample_diff(vec![
                sample_hunk("h1", 2),
                sample_hunk("h2", 1),
            ])),
        );
        navigation.toggle_hunk_collapsed(0, 0, 0);
        navigation.cursor = Some(LogCursor::Hunk {
            commit_index: 0,
            file_index: 0,
            hunk_index: 1,
        });

        // commit row (1) + metadata block + file row (1) + collapsed hunk header (1)
        // + separator (1) for the second hunk header.
        assert_eq!(
            navigation.cursor_flat_index(&log_data),
            Some(4 + metadata_lines)
        );
    }
}
