use crate::diff::Diff;
use crate::status::RepositoryStatus;
use std::cell::Cell;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionCursor {
    File {
        section: StatusSection,
        file_index: usize,
    },
    Hunk {
        section: StatusSection,
        file_index: usize,
        hunk_index: usize,
    },
}

#[derive(Debug, Clone)]
pub struct SelectedFile {
    pub path: String,
    pub context: FileContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileContext {
    Staged,
    Unstaged,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileDiffKey {
    pub path: String,
    pub context: FileContext,
}

impl FileDiffKey {
    pub fn new(path: String, context: FileContext) -> Self {
        Self { path, context }
    }
}

#[derive(Debug, Clone)]
pub enum OperationContext {
    CanStage,
    CanUnstage,
    CanAdd,
    ReadOnly,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct SectionCollapsedState {
    pub conflicted: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub staged: bool,
}

#[derive(Clone, Debug)]
pub struct InlineDiffState {
    pub file_path: String,
    pub file_context: FileContext,
    pub diff_context: crate::diff::DiffContext,
    pub diff: Option<Diff>,
    pub expanded: bool,
    pub current_hunk: usize,
    /// Tracks which hunks are collapsed (true = collapsed, false = expanded)
    /// Hunks are expanded by default
    pub collapsed_hunks: std::collections::HashSet<usize>,
}

pub struct NavigationState {
    cursor: Option<SelectionCursor>,
    sections: Vec<SectionInfo>,
    section_collapsed: SectionCollapsedState,
    file_diffs: HashMap<FileDiffKey, InlineDiffState>,
    scroll_offset: Cell<usize>,
    viewport_height: Cell<usize>,
    max_scroll_offset: Cell<usize>,
    manual_scroll_active: Cell<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusSection {
    Staged,
    Unstaged,
    Untracked,
    Conflicted,
}

impl From<StatusSection> for FileContext {
    fn from(section: StatusSection) -> Self {
        match section {
            StatusSection::Staged => FileContext::Staged,
            StatusSection::Unstaged => FileContext::Unstaged,
            StatusSection::Untracked => FileContext::Untracked,
            StatusSection::Conflicted => FileContext::Conflicted,
        }
    }
}

#[derive(Clone, Debug)]
struct SectionInfo {
    section_type: StatusSection,
}

impl NavigationState {
    pub fn new(status: &RepositoryStatus) -> Self {
        let sections = Self::build_sections(status);
        let cursor = Self::first_file_cursor_from_sections(&sections);

        Self {
            cursor,
            sections,
            section_collapsed: SectionCollapsedState::default(),
            file_diffs: HashMap::new(),
            scroll_offset: Cell::new(0),
            viewport_height: Cell::new(1),
            max_scroll_offset: Cell::new(0),
            manual_scroll_active: Cell::new(false),
        }
    }

    pub fn update_status(&mut self, status: &RepositoryStatus) {
        let new_sections = Self::build_sections(status);
        let prev_cursor = self.cursor;
        self.sections = new_sections;

        if let Some(cursor) = prev_cursor
            .and_then(|c| self.map_cursor_to_status(status, c))
            .or_else(|| Self::first_file_cursor_from_sections(&self.sections))
        {
            self.cursor = Some(cursor);
        }

        self.ensure_cursor_valid(status);
        self.reset_scroll_position();
        self.clear_manual_scroll();
    }

    pub fn move_to_previous(&mut self, status: &RepositoryStatus) {
        self.clear_manual_scroll();
        if let Some(cursor) = self.current_cursor() {
            if let Some(prev) = self.previous_cursor(status, &cursor) {
                self.apply_cursor(status, Some(prev));
            }
        } else {
            self.apply_cursor(status, self.first_file_cursor(status));
        }
    }

    pub fn move_to_next(&mut self, status: &RepositoryStatus) {
        self.clear_manual_scroll();
        if let Some(cursor) = self.current_cursor() {
            if let Some(next) = self.next_cursor(status, &cursor) {
                self.apply_cursor(status, Some(next));
            }
        } else {
            self.apply_cursor(status, self.first_file_cursor(status));
        }
    }

    pub fn move_to_top(&mut self, status: &RepositoryStatus) {
        self.clear_manual_scroll();
        self.apply_cursor(status, self.first_file_cursor(status));
    }

    pub fn move_to_bottom(&mut self, status: &RepositoryStatus) {
        self.clear_manual_scroll();
        self.apply_cursor(status, self.last_file_cursor(status));
    }

    pub fn current_section(&self) -> Option<StatusSection> {
        self.cursor.as_ref().map(|c| match c {
            SelectionCursor::File { section, .. } | SelectionCursor::Hunk { section, .. } => {
                *section
            }
        })
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.cursor.as_ref().map(|cursor| match cursor {
            SelectionCursor::File { file_index, .. } | SelectionCursor::Hunk { file_index, .. } => {
                *file_index
            }
        })
    }

    pub fn current_cursor(&self) -> Option<SelectionCursor> {
        self.cursor
    }

    pub fn cursor_position(&self) -> Option<(StatusSection, usize, Option<usize>)> {
        match self.cursor? {
            SelectionCursor::File {
                section,
                file_index,
            } => Some((section, file_index, None)),
            SelectionCursor::Hunk {
                section,
                file_index,
                hunk_index,
            } => Some((section, file_index, Some(hunk_index))),
        }
    }

    pub fn has_selections(&self) -> bool {
        self.cursor.is_some()
    }

    pub fn get_selected_file(&self, status: &RepositoryStatus) -> Option<SelectedFile> {
        let cursor = self.cursor?;
        let section = match cursor {
            SelectionCursor::File { section, .. } | SelectionCursor::Hunk { section, .. } => {
                section
            }
        };
        let file_index = match cursor {
            SelectionCursor::File { file_index, .. } | SelectionCursor::Hunk { file_index, .. } => {
                file_index
            }
        };

        let path = self.file_path(status, section, file_index)?;
        let context: FileContext = section.into();

        Some(SelectedFile { path, context })
    }

    pub fn get_operation_context(&self) -> OperationContext {
        match self.current_section() {
            Some(StatusSection::Staged) => OperationContext::CanUnstage,
            Some(StatusSection::Unstaged) => OperationContext::CanStage,
            Some(StatusSection::Untracked) => OperationContext::CanAdd,
            Some(StatusSection::Conflicted) => OperationContext::ReadOnly,
            None => OperationContext::ReadOnly,
        }
    }

    pub fn is_section_collapsed(&self, section: StatusSection) -> bool {
        match section {
            StatusSection::Conflicted => self.section_collapsed.conflicted,
            StatusSection::Unstaged => self.section_collapsed.unstaged,
            StatusSection::Untracked => self.section_collapsed.untracked,
            StatusSection::Staged => self.section_collapsed.staged,
        }
    }

    pub fn toggle_section_collapsed(&mut self, section: StatusSection) {
        match section {
            StatusSection::Conflicted => {
                self.section_collapsed.conflicted = !self.section_collapsed.conflicted
            }
            StatusSection::Unstaged => {
                self.section_collapsed.unstaged = !self.section_collapsed.unstaged
            }
            StatusSection::Untracked => {
                self.section_collapsed.untracked = !self.section_collapsed.untracked
            }
            StatusSection::Staged => self.section_collapsed.staged = !self.section_collapsed.staged,
        }
        self.clear_manual_scroll();
    }

    pub fn is_file_diff_expanded(&self, key: &FileDiffKey) -> bool {
        self.file_diffs
            .get(key)
            .map(|state| state.expanded)
            .unwrap_or(false)
    }

    pub fn has_cached_diff_for(
        &self,
        key: &FileDiffKey,
        context: &crate::diff::DiffContext,
    ) -> bool {
        self.file_diffs
            .get(key)
            .is_some_and(|state| state.diff.is_some() && state.diff_context == *context)
    }

    pub fn get_file_diff(&self, key: &FileDiffKey) -> Option<&InlineDiffState> {
        self.file_diffs.get(key)
    }

    pub fn toggle_file_diff_expanded(
        &mut self,
        key: FileDiffKey,
        diff_context: crate::diff::DiffContext,
    ) {
        if let Some(diff_state) = self.file_diffs.get_mut(&key) {
            diff_state.expanded = !diff_state.expanded;
        } else {
            self.file_diffs.insert(
                key.clone(),
                InlineDiffState {
                    file_path: key.path.clone(),
                    file_context: key.context,
                    diff_context,
                    diff: None,
                    expanded: true,
                    current_hunk: 0,
                    collapsed_hunks: std::collections::HashSet::new(),
                },
            );
        }
        self.clear_manual_scroll();
    }

    pub fn set_file_diff(
        &mut self,
        key: FileDiffKey,
        diff: Diff,
        diff_context: crate::diff::DiffContext,
    ) {
        self.file_diffs.insert(
            key.clone(),
            InlineDiffState {
                file_path: key.path.clone(),
                file_context: key.context,
                diff_context,
                diff: Some(diff),
                expanded: true,
                current_hunk: 0,
                collapsed_hunks: std::collections::HashSet::new(),
            },
        );
        self.clear_manual_scroll();
    }

    pub fn clear_diff_cache(&mut self) {
        self.file_diffs.clear();
    }

    pub fn reset_inline_diff_selection(&mut self, key: &FileDiffKey) {
        if let Some(state) = self.file_diffs.get_mut(key) {
            state.current_hunk = 0;
            self.clear_manual_scroll();
        }
    }

    pub fn toggle_hunk_collapsed(&mut self, key: &FileDiffKey, hunk_index: usize) {
        if let Some(state) = self.file_diffs.get_mut(key) {
            if state.collapsed_hunks.contains(&hunk_index) {
                state.collapsed_hunks.remove(&hunk_index);
            } else {
                state.collapsed_hunks.insert(hunk_index);
            }
            self.clear_manual_scroll();
        }
    }

    pub fn is_hunk_collapsed(&self, key: &FileDiffKey, hunk_index: usize) -> bool {
        self.file_diffs
            .get(key)
            .map(|state| state.collapsed_hunks.contains(&hunk_index))
            .unwrap_or(false)
    }

    pub fn remove_file_diff(&mut self, key: &FileDiffKey) {
        self.file_diffs.remove(key);
        self.clear_manual_scroll();
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset.get()
    }

    pub fn viewport_height(&self) -> usize {
        self.viewport_height.get()
    }

    pub fn set_scroll_offset(&self, offset: usize) {
        self.scroll_offset.set(offset);
    }

    pub fn scroll_viewport_up(&self, lines: usize) {
        if lines == 0 {
            return;
        }
        let current = self.scroll_offset.get();
        let new_offset = current.saturating_sub(lines);
        self.scroll_offset.set(new_offset);
        self.manual_scroll_active.set(true);
    }

    pub fn scroll_viewport_down(&self, lines: usize) {
        if lines == 0 {
            return;
        }
        let current = self.scroll_offset.get();
        let new_offset = current.saturating_add(lines);
        self.scroll_offset.set(new_offset);
        self.manual_scroll_active.set(true);
    }

    pub fn update_viewport_metrics(
        &self,
        viewport_height: usize,
        total_items: usize,
        visibility_targets: &[usize],
    ) {
        let clamped_height = viewport_height.max(1);
        self.viewport_height.set(clamped_height);
        let max_offset = total_items.saturating_sub(clamped_height);
        self.max_scroll_offset.set(max_offset);
        let clamped_offset = self.scroll_offset.get().min(max_offset);
        self.scroll_offset.set(clamped_offset);
        if !self.manual_scroll_active.get() {
            self.ensure_targets_visible(visibility_targets);
        }
    }

    pub fn is_manual_scroll_active(&self) -> bool {
        self.manual_scroll_active.get()
    }

    pub fn set_manual_scroll_active(&self, active: bool) {
        self.manual_scroll_active.set(active);
    }

    pub fn clear_manual_scroll(&self) {
        self.manual_scroll_active.set(false);
    }

    pub fn ensure_targets_visible(&self, visibility_targets: &[usize]) {
        let height = self.viewport_height.get().max(1);
        let max_offset = self.max_scroll_offset.get();
        let mut offset = self.scroll_offset.get().min(max_offset);

        if visibility_targets.is_empty() {
            if max_offset == 0 {
                offset = 0;
            }
            if offset > max_offset {
                offset = max_offset;
            }
            self.scroll_offset.set(offset);
            return;
        }

        let base_margin = 1usize;
        let margin = if height <= base_margin {
            0
        } else {
            base_margin
        };
        let mut effective_offset = offset;
        for &target in visibility_targets {
            if target < effective_offset {
                let desired = target.saturating_sub(margin);
                effective_offset = desired.min(max_offset);
            } else {
                let visible_end = effective_offset.saturating_add(height.saturating_sub(1));
                if target > visible_end {
                    let desired = target.saturating_sub(margin);
                    effective_offset = desired.min(max_offset);
                }
            }
        }

        let clamped_offset = if max_offset == 0 {
            0
        } else {
            effective_offset.min(max_offset)
        };

        self.scroll_offset.set(clamped_offset);
    }

    fn reset_scroll_position(&self) {
        self.scroll_offset.set(0);
        self.clear_manual_scroll();
    }

    fn section_order() -> [StatusSection; 4] {
        [
            StatusSection::Conflicted,
            StatusSection::Unstaged,
            StatusSection::Untracked,
            StatusSection::Staged,
        ]
    }

    fn build_sections(status: &RepositoryStatus) -> Vec<SectionInfo> {
        let mut sections = Vec::new();

        for section in Self::section_order() {
            let count = Self::section_file_count_for_status(status, section);
            if count > 0 {
                sections.push(SectionInfo {
                    section_type: section,
                });
            }
        }

        sections
    }

    fn section_file_count_for_status(status: &RepositoryStatus, section: StatusSection) -> usize {
        match section {
            StatusSection::Staged => status.staged_files().len(),
            StatusSection::Unstaged => status.unstaged_files().len(),
            StatusSection::Untracked => status.untracked_files().len(),
            StatusSection::Conflicted => status.conflicted_files().len(),
        }
    }

    fn file_path(
        &self,
        status: &RepositoryStatus,
        section: StatusSection,
        file_index: usize,
    ) -> Option<String> {
        match section {
            StatusSection::Staged => status
                .staged_files()
                .get(file_index)
                .map(|f| f.path.clone()),
            StatusSection::Unstaged => status
                .unstaged_files()
                .get(file_index)
                .map(|f| f.path.clone()),
            StatusSection::Untracked => status.untracked_files().get(file_index).cloned(),
            StatusSection::Conflicted => status.conflicted_files().get(file_index).cloned(),
        }
    }

    fn find_file_index(
        &self,
        status: &RepositoryStatus,
        section: StatusSection,
        path: &str,
    ) -> Option<usize> {
        match section {
            StatusSection::Staged => status.staged_files().iter().position(|f| f.path == path),
            StatusSection::Unstaged => status.unstaged_files().iter().position(|f| f.path == path),
            StatusSection::Untracked => status.untracked_files().iter().position(|f| f == path),
            StatusSection::Conflicted => status.conflicted_files().iter().position(|f| f == path),
        }
    }

    fn next_non_empty_section(
        &self,
        status: &RepositoryStatus,
        section: StatusSection,
    ) -> Option<StatusSection> {
        let order = Self::section_order();
        let current_pos = order.iter().position(|s| *s == section)?;
        order
            .iter()
            .skip(current_pos + 1)
            .copied()
            .find(|s| Self::section_file_count_for_status(status, *s) > 0)
    }

    fn previous_non_empty_section(
        &self,
        status: &RepositoryStatus,
        section: StatusSection,
    ) -> Option<StatusSection> {
        let order = Self::section_order();
        let current_pos = order.iter().position(|s| *s == section)?;
        order
            .iter()
            .take(current_pos)
            .copied()
            .rfind(|s| Self::section_file_count_for_status(status, *s) > 0)
    }

    pub fn section_file_count(&self, status: &RepositoryStatus, section: StatusSection) -> usize {
        Self::section_file_count_for_status(status, section)
    }

    pub fn hunk_count_for_file(
        &self,
        status: &RepositoryStatus,
        section: StatusSection,
        file_index: usize,
    ) -> usize {
        let Some(path) = self.file_path(status, section, file_index) else {
            return 0;
        };
        let key = FileDiffKey::new(path, section.into());
        if let Some(state) = self.file_diffs.get(&key) {
            if !state.expanded {
                return 0;
            }
            if let Some(diff) = &state.diff {
                return diff.hunks.len();
            }
        }
        0
    }

    fn first_file_cursor_from_sections(sections: &[SectionInfo]) -> Option<SelectionCursor> {
        sections.first().map(|section| SelectionCursor::File {
            section: section.section_type,
            file_index: 0,
        })
    }

    pub fn first_file_cursor(&self, status: &RepositoryStatus) -> Option<SelectionCursor> {
        if self.sections.is_empty() {
            return None;
        }
        Self::first_file_cursor_from_sections(&self.sections).or_else(|| {
            Self::section_order()
                .iter()
                .find(|s| Self::section_file_count_for_status(status, **s) > 0)
                .map(|section| SelectionCursor::File {
                    section: *section,
                    file_index: 0,
                })
        })
    }

    pub fn last_file_cursor(&self, status: &RepositoryStatus) -> Option<SelectionCursor> {
        let section = Self::section_order()
            .iter()
            .rev()
            .copied()
            .find(|s| Self::section_file_count_for_status(status, *s) > 0)?;

        let last_index = Self::section_file_count_for_status(status, section).saturating_sub(1);
        Some(SelectionCursor::File {
            section,
            file_index: last_index,
        })
    }

    fn map_cursor_to_status(
        &self,
        status: &RepositoryStatus,
        cursor: SelectionCursor,
    ) -> Option<SelectionCursor> {
        let (section, file_index) = match cursor {
            SelectionCursor::File {
                section,
                file_index,
            } => (section, file_index),
            SelectionCursor::Hunk {
                section,
                file_index,
                ..
            } => (section, file_index),
        };
        let path = self.file_path(status, section, file_index)?;
        let new_index = self.find_file_index(status, section, &path)?;

        if let SelectionCursor::Hunk { hunk_index, .. } = cursor {
            let hunk_count = self.hunk_count_for_file(status, section, new_index);
            if hunk_count > 0 {
                let clamped = hunk_index.min(hunk_count.saturating_sub(1));
                return Some(SelectionCursor::Hunk {
                    section,
                    file_index: new_index,
                    hunk_index: clamped,
                });
            }
        }

        Some(SelectionCursor::File {
            section,
            file_index: new_index,
        })
    }

    fn next_cursor(
        &self,
        status: &RepositoryStatus,
        cursor: &SelectionCursor,
    ) -> Option<SelectionCursor> {
        match *cursor {
            SelectionCursor::File {
                section,
                file_index,
            } => {
                if let Some(key) = self.file_diff_key_for(status, section, file_index)
                    && self.is_file_diff_expanded(&key)
                {
                    let hunk_count = self.hunk_count_for_file(status, section, file_index);
                    if hunk_count > 0 {
                        return Some(SelectionCursor::Hunk {
                            section,
                            file_index,
                            hunk_index: 0,
                        });
                    }
                }
                self.next_file_cursor(status, section, file_index)
            }
            SelectionCursor::Hunk {
                section,
                file_index,
                hunk_index,
            } => {
                let hunk_count = self.hunk_count_for_file(status, section, file_index);
                if hunk_index + 1 < hunk_count {
                    return Some(SelectionCursor::Hunk {
                        section,
                        file_index,
                        hunk_index: hunk_index + 1,
                    });
                }
                self.next_file_cursor(status, section, file_index)
            }
        }
    }

    fn next_file_cursor(
        &self,
        status: &RepositoryStatus,
        section: StatusSection,
        file_index: usize,
    ) -> Option<SelectionCursor> {
        let count = Self::section_file_count_for_status(status, section);
        if file_index + 1 < count {
            return Some(SelectionCursor::File {
                section,
                file_index: file_index + 1,
            });
        }

        let next_section = self.next_non_empty_section(status, section)?;
        Some(SelectionCursor::File {
            section: next_section,
            file_index: 0,
        })
    }

    fn previous_cursor(
        &self,
        status: &RepositoryStatus,
        cursor: &SelectionCursor,
    ) -> Option<SelectionCursor> {
        match *cursor {
            SelectionCursor::File {
                section,
                file_index,
            } => {
                if file_index == 0 {
                    let prev_section = self.previous_non_empty_section(status, section)?;
                    let prev_count = Self::section_file_count_for_status(status, prev_section);
                    if prev_count == 0 {
                        return None;
                    }
                    let prev_index = prev_count.saturating_sub(1);
                    if let Some(key) = self.file_diff_key_for(status, prev_section, prev_index)
                        && self.is_file_diff_expanded(&key)
                    {
                        let prev_hunk_count =
                            self.hunk_count_for_file(status, prev_section, prev_index);
                        if prev_hunk_count > 0 {
                            return Some(SelectionCursor::Hunk {
                                section: prev_section,
                                file_index: prev_index,
                                hunk_index: prev_hunk_count.saturating_sub(1),
                            });
                        }
                    }
                    return Some(SelectionCursor::File {
                        section: prev_section,
                        file_index: prev_index,
                    });
                }

                let prev_index = file_index - 1;
                if let Some(key) = self.file_diff_key_for(status, section, prev_index)
                    && self.is_file_diff_expanded(&key)
                {
                    let prev_hunk_count = self.hunk_count_for_file(status, section, prev_index);
                    if prev_hunk_count > 0 {
                        return Some(SelectionCursor::Hunk {
                            section,
                            file_index: prev_index,
                            hunk_index: prev_hunk_count.saturating_sub(1),
                        });
                    }
                }

                Some(SelectionCursor::File {
                    section,
                    file_index: prev_index,
                })
            }
            SelectionCursor::Hunk {
                section,
                file_index,
                hunk_index,
            } => {
                if hunk_index > 0 {
                    return Some(SelectionCursor::Hunk {
                        section,
                        file_index,
                        hunk_index: hunk_index - 1,
                    });
                }

                Some(SelectionCursor::File {
                    section,
                    file_index,
                })
            }
        }
    }

    pub fn apply_cursor(&mut self, status: &RepositoryStatus, cursor: Option<SelectionCursor>) {
        if let Some(SelectionCursor::Hunk {
            section,
            file_index,
            hunk_index,
        }) = cursor
            && let Some(key) = self.file_diff_key_for(status, section, file_index)
            && let Some(state) = self.file_diffs.get_mut(&key)
        {
            let max_index = state
                .diff
                .as_ref()
                .map(|d| d.hunks.len().saturating_sub(1))
                .unwrap_or(0);
            state.current_hunk = hunk_index.min(max_index);
        }

        self.cursor = cursor;
    }

    pub fn ensure_cursor_valid(&mut self, status: &RepositoryStatus) {
        let Some(cursor) = self.cursor else {
            self.cursor = self.first_file_cursor(status);
            return;
        };

        let (section, file_index) = match cursor {
            SelectionCursor::File {
                section,
                file_index,
            }
            | SelectionCursor::Hunk {
                section,
                file_index,
                ..
            } => (section, file_index),
        };

        let file_exists = file_index < Self::section_file_count_for_status(status, section)
            && self.file_path(status, section, file_index).is_some();
        if !file_exists {
            self.cursor = self.first_file_cursor(status);
            return;
        }

        if let SelectionCursor::Hunk {
            section,
            file_index,
            hunk_index,
        } = cursor
        {
            let hunk_count = self.hunk_count_for_file(status, section, file_index);
            if hunk_count == 0 {
                self.cursor = Some(SelectionCursor::File {
                    section,
                    file_index,
                });
                return;
            }

            if hunk_index >= hunk_count {
                self.cursor = Some(SelectionCursor::Hunk {
                    section,
                    file_index,
                    hunk_index: hunk_count.saturating_sub(1),
                });
                return;
            }
        }

        self.cursor = Some(cursor);
    }

    fn file_diff_key_for(
        &self,
        status: &RepositoryStatus,
        section: StatusSection,
        file_index: usize,
    ) -> Option<FileDiffKey> {
        let path = self.file_path(status, section, file_index)?;
        Some(FileDiffKey::new(path, section.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{Diff, DiffContext, DiffHunk, DiffLine, HunkHeader, LineRange, LineType};
    use crate::status::{FileEntry, FileStatus};

    fn create_file_entry(path: &str, status: FileStatus) -> FileEntry {
        FileEntry::new(path.to_string(), status)
    }

    fn create_test_status_with_files() -> RepositoryStatus {
        RepositoryStatus {
            branch_name: "main".to_string(),
            staged: vec![
                create_file_entry("staged1.txt", FileStatus::Added),
                create_file_entry("staged2.txt", FileStatus::Modified),
            ],
            unstaged: vec![
                create_file_entry("unstaged1.txt", FileStatus::Modified),
                create_file_entry("unstaged2.txt", FileStatus::Deleted),
                create_file_entry("unstaged3.txt", FileStatus::Modified),
            ],
            untracked: vec!["untracked1.txt".to_string()],
            conflicted: vec!["conflicted1.txt".to_string(), "conflicted2.txt".to_string()],
        }
    }

    fn create_single_section_status() -> RepositoryStatus {
        RepositoryStatus {
            branch_name: "main".to_string(),
            staged: vec![],
            unstaged: vec![
                create_file_entry("file1.txt", FileStatus::Modified),
                create_file_entry("file2.txt", FileStatus::Modified),
            ],
            untracked: vec![],
            conflicted: vec![],
        }
    }

    fn sample_hunk(label: &str) -> DiffHunk {
        DiffHunk {
            header: HunkHeader {
                raw: format!("@@ {label} @@"),
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
            },
            lines: vec![DiffLine {
                content: "line".into(),
                line_type: LineType::Context,
                old_line_no: Some(1),
                new_line_no: Some(1),
            }],
            old_range: LineRange { start: 1, count: 1 },
            new_range: LineRange { start: 1, count: 1 },
            stageable: true,
            context_lines: 0,
        }
    }

    #[test]
    fn initial_cursor_points_to_first_file() {
        let status = create_test_status_with_files();
        let nav = NavigationState::new(&status);

        assert!(matches!(
            nav.current_cursor(),
            Some(SelectionCursor::File {
                section: StatusSection::Conflicted,
                file_index: 0
            })
        ));
    }

    #[test]
    fn move_next_traverses_hunks_then_files() {
        let status = create_single_section_status();
        let mut nav = NavigationState::new(&status);
        let diff_key = FileDiffKey::new(
            status.unstaged_files()[0].path.clone(),
            FileContext::Unstaged,
        );
        let diff = Diff {
            file_path: diff_key.path.clone(),
            context: DiffContext::WorkingTreeToIndex,
            hunks: vec![sample_hunk("h1"), sample_hunk("h2")],
            binary: false,
        };
        nav.set_file_diff(diff_key.clone(), diff, DiffContext::WorkingTreeToIndex);

        nav.move_to_next(&status);
        assert!(matches!(
            nav.current_cursor(),
            Some(SelectionCursor::Hunk {
                section: StatusSection::Unstaged,
                file_index: 0,
                hunk_index: 0
            })
        ));

        nav.move_to_next(&status);
        assert!(matches!(
            nav.current_cursor(),
            Some(SelectionCursor::Hunk {
                section: StatusSection::Unstaged,
                file_index: 0,
                hunk_index: 1
            })
        ));

        nav.move_to_next(&status);
        assert!(matches!(
            nav.current_cursor(),
            Some(SelectionCursor::File {
                section: StatusSection::Unstaged,
                file_index: 1
            })
        ));
    }

    #[test]
    fn move_previous_steps_into_previous_file_hunk() {
        let status = create_single_section_status();
        let mut nav = NavigationState::new(&status);
        let diff_key = FileDiffKey::new(
            status.unstaged_files()[0].path.clone(),
            FileContext::Unstaged,
        );
        let diff = Diff {
            file_path: diff_key.path.clone(),
            context: DiffContext::WorkingTreeToIndex,
            hunks: vec![sample_hunk("h1"), sample_hunk("h2")],
            binary: false,
        };
        nav.set_file_diff(diff_key.clone(), diff, DiffContext::WorkingTreeToIndex);

        nav.move_to_bottom(&status);
        assert!(matches!(
            nav.current_cursor(),
            Some(SelectionCursor::File {
                section: StatusSection::Unstaged,
                file_index: 1
            })
        ));

        nav.move_to_previous(&status);
        assert!(matches!(
            nav.current_cursor(),
            Some(SelectionCursor::Hunk {
                section: StatusSection::Unstaged,
                file_index: 0,
                hunk_index: 1
            })
        ));

        nav.move_to_previous(&status);
        assert!(matches!(
            nav.current_cursor(),
            Some(SelectionCursor::Hunk {
                section: StatusSection::Unstaged,
                file_index: 0,
                hunk_index: 0
            })
        ));
    }

    #[test]
    fn hunk_cursor_downgrades_when_diff_missing() {
        let status = create_single_section_status();
        let mut nav = NavigationState::new(&status);
        let diff_key = FileDiffKey::new(
            status.unstaged_files()[0].path.clone(),
            FileContext::Unstaged,
        );
        nav.set_file_diff(
            diff_key.clone(),
            Diff {
                file_path: diff_key.path.clone(),
                context: DiffContext::WorkingTreeToIndex,
                hunks: vec![sample_hunk("h1")],
                binary: false,
            },
            DiffContext::WorkingTreeToIndex,
        );
        nav.apply_cursor(
            &status,
            Some(SelectionCursor::Hunk {
                section: StatusSection::Unstaged,
                file_index: 0,
                hunk_index: 0,
            }),
        );
        nav.clear_diff_cache();
        nav.ensure_cursor_valid(&status);

        assert!(matches!(
            nav.current_cursor(),
            Some(SelectionCursor::File {
                section: StatusSection::Unstaged,
                file_index: 0
            })
        ));
    }

    #[test]
    fn move_to_bottom_selects_last_file() {
        let status = create_test_status_with_files();
        let mut nav = NavigationState::new(&status);
        nav.move_to_bottom(&status);

        assert!(matches!(
            nav.current_cursor(),
            Some(SelectionCursor::File {
                section: StatusSection::Staged,
                file_index: 1
            })
        ));
    }

    #[test]
    fn page_scroll_down_increases_offset() {
        let status = create_single_section_status();
        let nav = NavigationState::new(&status);

        // Simulate viewport metrics
        nav.update_viewport_metrics(20, 100, &[]);

        // Initial offset should be 0
        assert_eq!(nav.scroll_offset(), 0);

        // Scroll down by one page (20 lines)
        nav.scroll_viewport_down(20);

        // Offset should increase
        assert_eq!(nav.scroll_offset(), 20);
        assert!(nav.is_manual_scroll_active());
    }

    #[test]
    fn page_scroll_down_continues_past_max_offset() {
        let status = create_single_section_status();
        let nav = NavigationState::new(&status);

        // Simulate viewport with 20 lines height and 50 total items
        // This means max_offset = 50 - 20 = 30
        nav.update_viewport_metrics(20, 50, &[]);

        // Scroll to near max_offset
        nav.scroll_viewport_down(25);
        assert_eq!(nav.scroll_offset(), 25);

        // Scroll again - should go to 45, which exceeds old max_offset of 30
        // but will be clamped to 30 by update_viewport_metrics
        nav.scroll_viewport_down(20);
        assert_eq!(nav.scroll_offset(), 45);

        // When update_viewport_metrics is called again, it should clamp to max_offset
        nav.update_viewport_metrics(20, 50, &[]);
        assert_eq!(nav.scroll_offset(), 30);
        assert!(nav.is_manual_scroll_active());
    }

    #[test]
    fn multiple_page_downs_then_page_ups() {
        let status = create_single_section_status();
        let nav = NavigationState::new(&status);

        nav.update_viewport_metrics(20, 100, &[]);

        // Press page down twice
        nav.scroll_viewport_down(20);
        nav.scroll_viewport_down(20);
        assert_eq!(nav.scroll_offset(), 40);

        // Press page up twice - should return to 0
        nav.scroll_viewport_up(20);
        assert_eq!(nav.scroll_offset(), 20);
        nav.scroll_viewport_up(20);
        assert_eq!(nav.scroll_offset(), 0);
    }

    #[test]
    fn manual_scroll_persists_across_page_scrolls() {
        let status = create_single_section_status();
        let nav = NavigationState::new(&status);

        nav.update_viewport_metrics(20, 100, &[]);

        assert!(!nav.is_manual_scroll_active());

        nav.scroll_viewport_down(20);
        assert!(nav.is_manual_scroll_active());

        nav.scroll_viewport_down(20);
        assert!(nav.is_manual_scroll_active());

        nav.scroll_viewport_up(10);
        assert!(nav.is_manual_scroll_active());
    }

    #[test]
    fn scroll_viewport_down_without_clamping() {
        let status = create_single_section_status();
        let nav = NavigationState::new(&status);

        // Set up viewport with max_offset = 30
        nav.update_viewport_metrics(20, 50, &[]);

        // Scroll to exactly max_offset
        nav.scroll_viewport_down(30);
        assert_eq!(nav.scroll_offset(), 30);

        // Scroll again - should increase offset even though it exceeds max_offset
        // (it will be clamped by update_viewport_metrics later)
        nav.scroll_viewport_down(20);
        assert_eq!(nav.scroll_offset(), 50);

        // This demonstrates that scroll_viewport_down doesn't clamp,
        // allowing continuous scrolling
    }
}
