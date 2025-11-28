use crate::config::Config;
use crate::diff::generator::DiffError;
use crate::diff::{Diff, HunkPosition};
use crate::ui::diff_renderer::DiffRenderer;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

pub enum DiffViewContent {
    Success(Diff),
    Error { file_path: String, error: DiffError },
}

pub struct DiffView {
    content: DiffViewContent,
    scroll_position: usize,
    viewport_height: usize,
    current_hunk_index: Option<usize>,
    config: Config,
    hunk_positions: Vec<HunkPosition>,
}

impl DiffView {
    pub fn new(diff: Diff, config: Config) -> Self {
        let current_hunk_index = if diff.hunks.is_empty() { None } else { Some(0) };
        let hunk_positions = Self::calculate_hunk_positions_static(&diff);
        Self {
            content: DiffViewContent::Success(diff),
            scroll_position: 0,
            viewport_height: 0,
            current_hunk_index,
            config,
            hunk_positions,
        }
    }

    pub fn new_with_error(file_path: String, error: DiffError, config: Config) -> Self {
        Self {
            content: DiffViewContent::Error { file_path, error },
            scroll_position: 0,
            viewport_height: 0,
            current_hunk_index: None,
            config,
            hunk_positions: Vec::new(),
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        match &self.content {
            DiffViewContent::Success(diff) => {
                if diff.binary {
                    self.render_binary_message(frame, area, &diff.file_path);
                } else {
                    self.render_diff_content(frame, area, diff);
                }
            }
            DiffViewContent::Error { file_path, error } => {
                self.render_error_message(frame, area, file_path, error);
            }
        }
    }

    fn render_binary_message(&self, frame: &mut Frame, area: Rect, file_path: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} (binary) ", file_path));

        let text = Text::from(vec![Line::from(Span::styled(
            "Binary file - cannot display diff",
            Style::default().fg(self.config.theme.diff_no_newline),
        ))]);

        let paragraph = Paragraph::new(text).block(block);
        frame.render_widget(paragraph, area);
    }

    fn render_error_message(
        &self,
        frame: &mut Frame,
        area: Rect,
        file_path: &str,
        error: &DiffError,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} (error) ", file_path));

        let (message, color) = match error {
            DiffError::BinaryFile(_) => (
                "Binary file - cannot display diff".to_string(),
                self.config.theme.diff_no_newline,
            ),
            DiffError::FileTooLarge(size, _) => (
                format!("File too large ({} bytes) - cannot display diff", size),
                self.config.theme.unstaged,
            ),
            DiffError::TerminalCompatibility(msg) => (
                format!("Terminal compatibility issue: {}", msg),
                self.config.theme.untracked,
            ),
            DiffError::FileNotFound(_) => {
                ("File not found".to_string(), self.config.theme.unstaged)
            }
            DiffError::Git(git_err) => (
                format!("Git error: {}", git_err),
                self.config.theme.unstaged,
            ),
            DiffError::Io(io_err) => (format!("I/O error: {}", io_err), self.config.theme.unstaged),
        };

        let text = Text::from(vec![Line::from(Span::styled(
            message,
            Style::default().fg(color),
        ))]);

        let paragraph = Paragraph::new(text).block(block);
        frame.render_widget(paragraph, area);
    }

    fn render_diff_content(&self, frame: &mut Frame, area: Rect, diff: &Diff) {
        // Create block with title
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", diff.file_path));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        if inner_area.height == 0 || inner_area.width == 0 {
            return;
        }

        let viewport_height = inner_area.height as usize;

        let active_hunk_index = self.resolve_active_hunk_index(&self.hunk_positions);

        // Generate diff lines for display
        let diff_lines = self.generate_diff_lines(diff, active_hunk_index);
        let total_lines = diff_lines.len();

        // Calculate visible range and determine if we need sticky header
        let visible_start = self.scroll_position.min(total_lines);
        let pinned_header_line = active_hunk_index
            .and_then(|idx| self.hunk_positions.get(idx))
            .map(|pos| pos.start_line);
        let should_pin_header = pinned_header_line
            .map(|line| line < visible_start)
            .unwrap_or(false);

        // Only reduce viewport height if we're actually showing the sticky header
        let content_viewport_height = if should_pin_header {
            viewport_height.saturating_sub(1)
        } else {
            viewport_height
        };

        let visible_lines = self.collect_visible_lines(
            &diff_lines,
            visible_start,
            content_viewport_height,
            if should_pin_header {
                pinned_header_line
            } else {
                None
            },
        );

        let mut content_area_for_lines = inner_area;

        if should_pin_header
            && let Some(active_index) = active_hunk_index
            && content_area_for_lines.height > 0
        {
            let header_area = Rect {
                x: content_area_for_lines.x,
                y: content_area_for_lines.y,
                width: content_area_for_lines.width,
                height: 1,
            };
            self.render_sticky_header(frame, header_area, diff, active_index);
            if content_area_for_lines.height > 1 {
                content_area_for_lines.y += 1;
                content_area_for_lines.height -= 1;
            } else {
                content_area_for_lines.height = 0;
            }
        }

        if content_area_for_lines.height > 0 {
            let text = Text::from(visible_lines);
            let paragraph = Paragraph::new(text);
            frame.render_widget(paragraph, content_area_for_lines);
        }

        // Render scrollbar if content is scrollable
        if total_lines > viewport_height {
            self.render_scrollbar(frame, area, total_lines, viewport_height);
        }
    }

    fn generate_diff_lines(
        &self,
        diff: &Diff,
        active_hunk_index: Option<usize>,
    ) -> Vec<Line<'static>> {
        let renderer = DiffRenderer::new(&self.config);
        renderer.generate_diff_lines(diff, active_hunk_index, None)
    }

    fn render_scrollbar(
        &self,
        frame: &mut Frame,
        area: Rect,
        total_lines: usize,
        viewport_height: usize,
    ) {
        let scrollbar_area = Rect {
            x: area.x + area.width - 1,
            y: area.y + 1,
            width: 1,
            height: area.height - 2,
        };

        let mut scrollbar_state = ScrollbarState::default()
            .content_length(total_lines)
            .viewport_content_length(viewport_height)
            .position(self.scroll_position);

        let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);

        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_position = self.scroll_position.saturating_sub(lines);
        self.sync_current_hunk_from_scroll();
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let max_scroll = self.get_max_scroll_position();
        self.scroll_position = (self.scroll_position + lines).min(max_scroll);
        self.sync_current_hunk_from_scroll();
    }

    pub fn page_up(&mut self) {
        let page_size = self.viewport_height.saturating_sub(2); // Leave some overlap
        self.scroll_up(page_size);
    }

    pub fn page_down(&mut self) {
        let page_size = self.viewport_height.saturating_sub(2); // Leave some overlap
        self.scroll_down(page_size);
    }

    pub fn jump_to_next_hunk(&mut self) {
        if let DiffViewContent::Success(diff) = &self.content {
            if diff.hunks.is_empty() {
                return;
            }

            // Find next hunk based on current scroll position
            let current_line = self.scroll_position;
            let mut line_count = 0;
            let mut found_current = false;

            for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
                if line_count > current_line && !found_current {
                    self.current_hunk_index = Some(hunk_index);
                    self.scroll_position = line_count;
                    return;
                }
                if line_count >= current_line {
                    found_current = true;
                }
                line_count += 1 + hunk.lines.len(); // Header + lines
            }

            // If no next hunk found, go to first hunk
            if found_current {
                self.current_hunk_index = Some(0);
                self.scroll_position = 0;
            }
        }
    }

    pub fn jump_to_previous_hunk(&mut self) {
        if let DiffViewContent::Success(diff) = &self.content {
            if diff.hunks.is_empty() {
                return;
            }

            let current_line = self.scroll_position;
            let mut line_count = 0;
            let mut last_hunk_start = 0;
            let mut last_hunk_index = 0;

            for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
                if line_count >= current_line && last_hunk_start < current_line {
                    self.current_hunk_index = Some(last_hunk_index);
                    self.scroll_position = last_hunk_start;
                    return;
                }
                last_hunk_start = line_count;
                last_hunk_index = hunk_index;
                line_count += 1 + hunk.lines.len(); // Header + lines
            }

            // If we're at or past the last hunk, go to the last hunk
            if current_line > 0 {
                self.current_hunk_index = Some(diff.hunks.len() - 1);
                self.scroll_position = last_hunk_start;
            }
        }
    }

    pub fn go_to_top(&mut self) {
        self.scroll_position = 0;
        self.sync_current_hunk_from_scroll();
    }

    pub fn go_to_bottom(&mut self) {
        let max_scroll = self.get_max_scroll_position();
        self.scroll_position = max_scroll;
        self.sync_current_hunk_from_scroll();
    }

    pub fn update_viewport_height(&mut self, area_height: u16) {
        self.viewport_height = area_height.saturating_sub(2) as usize;
    }

    fn get_max_scroll_position(&self) -> usize {
        let total_lines = self.get_total_lines();
        let effective_viewport = if self.has_hunks() {
            self.viewport_height.saturating_sub(1)
        } else {
            self.viewport_height
        };
        if effective_viewport == 0 {
            // When viewport collapses, still bound scroll to at most total_lines - 1
            // to prevent getting stuck past the end when viewport expands again
            total_lines.saturating_sub(1)
        } else {
            total_lines.saturating_sub(effective_viewport)
        }
    }

    fn get_total_lines(&self) -> usize {
        match &self.content {
            DiffViewContent::Success(diff) => {
                diff.hunks
                    .iter()
                    .map(|hunk| 1 + hunk.lines.len()) // 1 for header + lines
                    .sum()
            }
            DiffViewContent::Error { .. } => 0,
        }
    }

    pub fn get_hunk_count(&self) -> usize {
        match &self.content {
            DiffViewContent::Success(diff) => diff.hunks.len(),
            DiffViewContent::Error { .. } => 0,
        }
    }

    pub fn get_current_hunk_index(&self) -> Option<usize> {
        self.current_hunk_index
    }

    pub fn navigate_to_next_hunk(&mut self) {
        if let DiffViewContent::Success(diff) = &self.content {
            if diff.hunks.is_empty() {
                return;
            }

            match self.current_hunk_index {
                Some(current) => {
                    let next_index = (current + 1) % diff.hunks.len();
                    self.current_hunk_index = Some(next_index);
                    self.scroll_to_hunk(next_index);
                }
                None => {
                    self.current_hunk_index = Some(0);
                    self.scroll_to_hunk(0);
                }
            }
        }
    }

    pub fn navigate_to_previous_hunk(&mut self) {
        if let DiffViewContent::Success(diff) = &self.content {
            if diff.hunks.is_empty() {
                return;
            }

            match self.current_hunk_index {
                Some(current) => {
                    let prev_index = if current == 0 {
                        diff.hunks.len() - 1
                    } else {
                        current - 1
                    };
                    self.current_hunk_index = Some(prev_index);
                    self.scroll_to_hunk(prev_index);
                }
                None => {
                    let last_index = diff.hunks.len() - 1;
                    self.current_hunk_index = Some(last_index);
                    self.scroll_to_hunk(last_index);
                }
            }
        }
    }

    fn scroll_to_hunk(&mut self, hunk_index: usize) {
        if let DiffViewContent::Success(diff) = &self.content {
            if hunk_index >= diff.hunks.len() {
                return;
            }

            let mut line_count = 0;
            for (i, hunk) in diff.hunks.iter().enumerate() {
                if i == hunk_index {
                    self.scroll_position = line_count;
                    return;
                }
                line_count += 1 + hunk.lines.len(); // 1 for header + lines
            }
        }
    }

    fn calculate_hunk_positions_static(diff: &Diff) -> Vec<HunkPosition> {
        let mut positions = Vec::with_capacity(diff.hunks.len());
        let mut current_line = 0;

        for hunk in &diff.hunks {
            let start_line = current_line;
            let hunk_line_count = 1 + hunk.lines.len();
            let end_line = current_line + hunk_line_count;

            positions.push(HunkPosition {
                start_line,
                end_line,
                screen_y: start_line,
            });

            current_line = end_line;
        }

        positions
    }

    fn resolve_active_hunk_index(&self, positions: &[HunkPosition]) -> Option<usize> {
        let stored_index = self.current_hunk_index.filter(|idx| *idx < positions.len());

        stored_index.or_else(|| self.find_hunk_for_line(positions, self.scroll_position))
    }

    fn find_hunk_for_line(&self, positions: &[HunkPosition], line_index: usize) -> Option<usize> {
        // Binary search to find the hunk containing line_index
        positions
            .binary_search_by(|pos| {
                if line_index < pos.start_line {
                    std::cmp::Ordering::Greater
                } else if line_index >= pos.end_line {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok()
    }

    fn collect_visible_lines(
        &self,
        diff_lines: &[Line<'static>],
        visible_start: usize,
        content_height: usize,
        skip_line: Option<usize>,
    ) -> Vec<Line<'static>> {
        if content_height == 0 {
            return Vec::new();
        }

        let mut lines = Vec::with_capacity(content_height);
        let mut index = visible_start;

        while lines.len() < content_height && index < diff_lines.len() {
            if Some(index) == skip_line {
                index += 1;
                continue;
            }
            lines.push(diff_lines[index].clone());
            index += 1;
        }

        lines
    }

    fn render_sticky_header(
        &self,
        frame: &mut Frame,
        area: Rect,
        diff: &Diff,
        active_hunk_index: usize,
    ) {
        if let Some(hunk) = diff.hunks.get(active_hunk_index) {
            let header_line = Line::from(Span::styled(
                hunk.header.raw.clone(),
                self.config.theme.diff_hunk_header,
            ));
            let paragraph = Paragraph::new(header_line);
            frame.render_widget(paragraph, area);
        }
    }

    fn has_hunks(&self) -> bool {
        matches!(&self.content, DiffViewContent::Success(diff) if !diff.hunks.is_empty())
    }

    fn sync_current_hunk_from_scroll(&mut self) {
        if let DiffViewContent::Success(_) = &self.content {
            self.current_hunk_index =
                self.find_hunk_for_line(&self.hunk_positions, self.scroll_position);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{DiffContext, DiffHunk, DiffLine, HunkHeader, LineRange, LineType};

    fn create_test_diff() -> Diff {
        let hunk = DiffHunk {
            header: HunkHeader {
                raw: "@@ -1,3 +1,4 @@".to_string(),
                old_start: 1,
                old_lines: 3,
                new_start: 1,
                new_lines: 4,
            },
            old_range: LineRange { start: 1, count: 3 },
            new_range: LineRange { start: 1, count: 4 },
            stageable: true,
            context_lines: 3,
            lines: vec![
                DiffLine {
                    content: "line 1".to_string(),
                    line_type: LineType::Context,
                    old_line_no: Some(1),
                    new_line_no: Some(1),
                    inline_diff: None,
                },
                DiffLine {
                    content: "old line".to_string(),
                    line_type: LineType::Deletion,
                    old_line_no: Some(2),
                    new_line_no: None,
                    inline_diff: None,
                },
                DiffLine {
                    content: "new line".to_string(),
                    line_type: LineType::Addition,
                    old_line_no: None,
                    new_line_no: Some(2),
                    inline_diff: None,
                },
            ],
        };

        Diff {
            file_path: "test.txt".to_string(),
            context: DiffContext::WorkingTreeToIndex,
            hunks: vec![hunk],
            binary: false,
        }
    }

    #[test]
    fn test_diff_view_creation() {
        let diff = create_test_diff();
        let config = Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let diff_view = DiffView::new(diff, config);
        assert_eq!(diff_view.scroll_position, 0);
    }

    #[test]
    fn test_scrolling() {
        let diff = create_test_diff();
        let config = Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let mut diff_view = DiffView::new(diff, config);
        diff_view.viewport_height = 2; // Small viewport to enable scrolling

        // Test scroll down
        diff_view.scroll_down(2);
        assert_eq!(diff_view.scroll_position, 2);

        // Test scroll up
        diff_view.scroll_up(1);
        assert_eq!(diff_view.scroll_position, 1);

        // Test go to top
        diff_view.go_to_top();
        assert_eq!(diff_view.scroll_position, 0);
    }

    #[test]
    fn test_binary_diff() {
        let mut diff = create_test_diff();
        diff.binary = true;
        let config = Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let diff_view = DiffView::new(diff, config);
        match &diff_view.content {
            DiffViewContent::Success(diff) => assert!(diff.binary),
            _ => panic!("Expected successful diff content"),
        }
    }

    #[test]
    fn test_hunk_navigation() {
        let diff = create_test_diff();
        let config = Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let mut diff_view = DiffView::new(diff, config);
        diff_view.viewport_height = 10;

        // Should start at position 0
        assert_eq!(diff_view.scroll_position, 0);

        // Jump to next hunk should not move (only one hunk)
        diff_view.jump_to_next_hunk();
        assert_eq!(diff_view.scroll_position, 0);

        // Jump to previous hunk should not move
        diff_view.jump_to_previous_hunk();
        assert_eq!(diff_view.scroll_position, 0);
    }

    #[test]
    fn test_arrow_key_navigation() {
        let diff = create_test_diff();
        let config = Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let mut diff_view = DiffView::new(diff, config);
        diff_view.viewport_height = 10;

        // Should start with first hunk selected
        assert_eq!(diff_view.current_hunk_index, Some(0));
        assert_eq!(diff_view.scroll_position, 0);

        // Navigate to next hunk (wraps around since only one hunk)
        diff_view.navigate_to_next_hunk();
        assert_eq!(diff_view.current_hunk_index, Some(0));
        assert_eq!(diff_view.scroll_position, 0);

        // Navigate to previous hunk (wraps around since only one hunk)
        diff_view.navigate_to_previous_hunk();
        assert_eq!(diff_view.current_hunk_index, Some(0));
        assert_eq!(diff_view.scroll_position, 0);
    }

    fn create_multi_hunk_diff() -> Diff {
        let hunk1 = DiffHunk {
            header: HunkHeader {
                raw: "@@ -1,3 +1,4 @@".to_string(),
                old_start: 1,
                old_lines: 3,
                new_start: 1,
                new_lines: 4,
            },
            old_range: LineRange { start: 1, count: 3 },
            new_range: LineRange { start: 1, count: 4 },
            stageable: true,
            context_lines: 3,
            lines: vec![
                DiffLine {
                    content: "line 1".to_string(),
                    line_type: LineType::Context,
                    old_line_no: Some(1),
                    new_line_no: Some(1),
                    inline_diff: None,
                },
                DiffLine {
                    content: "old line".to_string(),
                    line_type: LineType::Deletion,
                    old_line_no: Some(2),
                    new_line_no: None,
                    inline_diff: None,
                },
            ],
        };

        let hunk2 = DiffHunk {
            header: HunkHeader {
                raw: "@@ -10,2 +11,3 @@".to_string(),
                old_start: 10,
                old_lines: 2,
                new_start: 11,
                new_lines: 3,
            },
            old_range: LineRange {
                start: 10,
                count: 2,
            },
            new_range: LineRange {
                start: 11,
                count: 3,
            },
            stageable: true,
            context_lines: 3,
            lines: vec![
                DiffLine {
                    content: "line 10".to_string(),
                    line_type: LineType::Context,
                    old_line_no: Some(10),
                    new_line_no: Some(11),
                    inline_diff: None,
                },
                DiffLine {
                    content: "new line".to_string(),
                    line_type: LineType::Addition,
                    old_line_no: None,
                    new_line_no: Some(12),
                    inline_diff: None,
                },
            ],
        };

        Diff {
            file_path: "test.txt".to_string(),
            context: DiffContext::WorkingTreeToIndex,
            hunks: vec![hunk1, hunk2],
            binary: false,
        }
    }

    #[test]
    fn test_multi_hunk_navigation() {
        let diff = create_multi_hunk_diff();
        let config = Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let mut diff_view = DiffView::new(diff, config);
        diff_view.viewport_height = 10;

        // Should start with first hunk selected
        assert_eq!(diff_view.current_hunk_index, Some(0));
        assert_eq!(diff_view.scroll_position, 0);

        // Navigate to next hunk
        diff_view.navigate_to_next_hunk();
        assert_eq!(diff_view.current_hunk_index, Some(1));
        // Should scroll to second hunk (1 header + 2 lines = 3 lines offset)
        assert_eq!(diff_view.scroll_position, 3);

        // Navigate to next hunk (should wrap to first)
        diff_view.navigate_to_next_hunk();
        assert_eq!(diff_view.current_hunk_index, Some(0));
        assert_eq!(diff_view.scroll_position, 0);

        // Navigate to previous hunk (should wrap to last)
        diff_view.navigate_to_previous_hunk();
        assert_eq!(diff_view.current_hunk_index, Some(1));
        assert_eq!(diff_view.scroll_position, 3);
    }

    #[test]
    fn test_scroll_updates_active_hunk() {
        let diff = create_multi_hunk_diff();
        let config = Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let mut diff_view = DiffView::new(diff, config);
        diff_view.viewport_height = 4;

        assert_eq!(diff_view.get_current_hunk_index(), Some(0));

        // Move past the first hunk (header + 2 lines)
        diff_view.scroll_down(3);
        assert_eq!(diff_view.get_current_hunk_index(), Some(1));

        // Scroll back up to return to the first hunk
        diff_view.scroll_up(1);
        assert_eq!(diff_view.get_current_hunk_index(), Some(0));
    }

    #[test]
    fn test_hunk_count() {
        let single_hunk_diff = create_test_diff();
        let theme = crate::theme::Theme::from_name("github-dark").unwrap();
        let config = Config {
            theme: theme.clone(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let single_hunk_view = DiffView::new(single_hunk_diff, config.clone());
        assert_eq!(single_hunk_view.get_hunk_count(), 1);

        let multi_hunk_diff = create_multi_hunk_diff();
        let multi_hunk_view = DiffView::new(multi_hunk_diff, config.clone());
        assert_eq!(multi_hunk_view.get_hunk_count(), 2);

        let empty_diff = Diff {
            file_path: "empty.txt".to_string(),
            context: DiffContext::WorkingTreeToIndex,
            hunks: vec![],
            binary: false,
        };
        let empty_view = DiffView::new(empty_diff, config);
        assert_eq!(empty_view.get_hunk_count(), 0);
        assert_eq!(empty_view.get_current_hunk_index(), None);
    }

    #[test]
    fn test_line_numbers_toggle_and_gutter() {
        let diff = create_test_diff();
        let theme = crate::theme::Theme::from_name("github-dark").unwrap();
        let mut config = Config {
            theme: theme.clone(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let diff_view = DiffView::new(diff.clone(), config.clone());

        let lines_with_numbers = diff_view.generate_diff_lines(&diff, diff_view.current_hunk_index);
        // First line after header belongs to the current hunk
        let first_content_line = &lines_with_numbers[1];
        // With syntax highlighting enabled, the span count may vary (line prefix + highlighted segments)
        // but the key elements (line numbers and gutter) should still be present
        assert!(first_content_line.spans.len() >= 7);
        assert_eq!(first_content_line.spans[0].content, "1");
        assert_eq!(first_content_line.spans[2].content, "1");
        assert_eq!(first_content_line.spans[4].content, "|");

        config.show_line_numbers = false;
        let diff_view_no_numbers = DiffView::new(diff.clone(), config);
        let lines_without_numbers = diff_view_no_numbers
            .generate_diff_lines(&diff, diff_view_no_numbers.current_hunk_index);
        let first_line_without_numbers = &lines_without_numbers[1];
        // With syntax highlighting, there may be more than 3 spans (gutter + space + prefix + content segments)
        assert!(first_line_without_numbers.spans.len() >= 3);
        assert_eq!(first_line_without_numbers.spans[0].content, "|");
    }

    #[test]
    fn test_current_hunk_highlight_applies_to_lines() {
        let diff = create_multi_hunk_diff();
        let theme = crate::theme::Theme::from_name("github-dark").unwrap();
        let config = Config {
            theme: theme.clone(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let mut diff_view = DiffView::new(diff.clone(), config.clone());

        let lines = diff_view.generate_diff_lines(&diff, diff_view.current_hunk_index);
        let first_hunk_line = &lines[1];
        assert_eq!(
            first_hunk_line.spans[0].style.bg,
            Some(config.theme.diff_hunk_highlight)
        );

        diff_view.current_hunk_index = Some(1);
        let lines_second_hunk_selected =
            diff_view.generate_diff_lines(&diff, diff_view.current_hunk_index);
        let first_hunk_line_after_switch = &lines_second_hunk_selected[1];
        assert_eq!(first_hunk_line_after_switch.spans[0].style.bg, None);

        // Second hunk header sits at index 3, so the first content line is at index 4
        let second_hunk_line = &lines_second_hunk_selected[4];
        assert_eq!(
            second_hunk_line.spans[0].style.bg,
            Some(config.theme.diff_hunk_highlight)
        );
    }

    #[test]
    fn test_should_pin_header_when_scrolled_past_header() {
        let diff = create_multi_hunk_diff();
        let config = Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let mut diff_view = DiffView::new(diff, config);
        diff_view.viewport_height = 5;

        // At top (scroll_position = 0), header should not be pinned
        let active_hunk_index = diff_view.resolve_active_hunk_index(&diff_view.hunk_positions);
        let pinned_header_line = active_hunk_index
            .and_then(|idx| diff_view.hunk_positions.get(idx))
            .map(|pos| pos.start_line);
        let should_pin_at_top = pinned_header_line
            .map(|line| line < diff_view.scroll_position)
            .unwrap_or(false);
        assert!(
            !should_pin_at_top,
            "Header should not be pinned at scroll position 0"
        );

        // Scroll past first hunk header (line 0)
        diff_view.scroll_down(2);
        let active_hunk_index = diff_view.resolve_active_hunk_index(&diff_view.hunk_positions);
        let pinned_header_line = active_hunk_index
            .and_then(|idx| diff_view.hunk_positions.get(idx))
            .map(|pos| pos.start_line);
        let should_pin_scrolled = pinned_header_line
            .map(|line| line < diff_view.scroll_position)
            .unwrap_or(false);
        assert!(
            should_pin_scrolled,
            "Header should be pinned when scrolled past it"
        );
        assert_eq!(pinned_header_line, Some(0), "First hunk starts at line 0");
    }

    #[test]
    fn test_collect_visible_lines_skips_header_when_pinned() {
        let diff = create_multi_hunk_diff();
        let config = Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let diff_view = DiffView::new(diff.clone(), config);

        let diff_lines = diff_view.generate_diff_lines(&diff, Some(0));

        // Without pinning: collect lines starting at position 1, should include line at index 1
        let visible_no_skip = diff_view.collect_visible_lines(&diff_lines, 1, 3, None);
        assert_eq!(visible_no_skip.len(), 3);
        // Line at index 1 should be included
        assert!(
            visible_no_skip[0]
                .spans
                .iter()
                .any(|s| s.content.contains("line 1"))
        );

        // With pinning: collect lines starting at position 1, but skip line 0 (the header)
        let visible_with_skip = diff_view.collect_visible_lines(&diff_lines, 1, 3, Some(0));
        assert_eq!(visible_with_skip.len(), 3);
        // Still starts at line 1, but we didn't lose any lines because we're not skipping our start line
        assert!(
            visible_with_skip[0]
                .spans
                .iter()
                .any(|s| s.content.contains("line 1"))
        );

        // Test skipping a line within the visible range
        let visible_skip_within = diff_view.collect_visible_lines(&diff_lines, 0, 3, Some(1));
        assert_eq!(visible_skip_within.len(), 3);
        // Should have lines at indices 0, 2, 3 (skipping index 1)
        // First line should be the header
        assert!(
            visible_skip_within[0]
                .spans
                .iter()
                .any(|s| s.content.contains("@@"))
        );
        // Second line should be from index 2 (the deletion line)
        assert!(
            visible_skip_within[1]
                .spans
                .iter()
                .any(|s| s.content.contains("old line"))
        );
    }

    #[test]
    fn test_sticky_header_viewport_height_adjustment() {
        let diff = create_multi_hunk_diff();
        let config = Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
        };
        let mut diff_view = DiffView::new(diff, config);
        diff_view.viewport_height = 5;

        // Scroll past the first hunk header
        diff_view.scroll_down(2);

        // Verify the viewport height calculation
        // When sticky header is shown, content viewport should be reduced by 1
        let active_hunk_index = diff_view.resolve_active_hunk_index(&diff_view.hunk_positions);
        let pinned_header_line = active_hunk_index
            .and_then(|idx| diff_view.hunk_positions.get(idx))
            .map(|pos| pos.start_line);
        let should_pin = pinned_header_line
            .map(|line| line < diff_view.scroll_position)
            .unwrap_or(false);

        let content_viewport_height = if should_pin {
            diff_view.viewport_height.saturating_sub(1)
        } else {
            diff_view.viewport_height
        };

        assert!(should_pin, "Header should be pinned");
        assert_eq!(
            content_viewport_height, 4,
            "Content viewport should be reduced by 1 when header is pinned"
        );
        assert_eq!(
            diff_view.viewport_height, 5,
            "Original viewport height should remain unchanged"
        );
    }
}
