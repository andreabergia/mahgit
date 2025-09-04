use crate::diff::generator::DiffError;
use crate::diff::{Diff, DiffLine, DiffLineType};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
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
}

impl DiffView {
    pub fn new(diff: Diff) -> Self {
        let current_hunk_index = if diff.hunks.is_empty() { None } else { Some(0) };
        Self {
            content: DiffViewContent::Success(diff),
            scroll_position: 0,
            viewport_height: 0,
            current_hunk_index,
        }
    }

    pub fn new_with_error(file_path: String, error: DiffError) -> Self {
        Self {
            content: DiffViewContent::Error { file_path, error },
            scroll_position: 0,
            viewport_height: 0,
            current_hunk_index: None,
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
            Style::default().fg(Color::Yellow),
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
                Color::Yellow,
            ),
            DiffError::FileTooLarge(size, _) => (
                format!("File too large ({} bytes) - cannot display diff", size),
                Color::Red,
            ),
            DiffError::TerminalCompatibility(msg) => (
                format!("Terminal compatibility issue: {}", msg),
                Color::Magenta,
            ),
            DiffError::FileNotFound(_) => ("File not found".to_string(), Color::Red),
            DiffError::Git(git_err) => (format!("Git error: {}", git_err), Color::Red),
            DiffError::Io(io_err) => (format!("I/O error: {}", io_err), Color::Red),
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

        // Calculate viewport height
        let viewport_height = area.height.saturating_sub(2) as usize; // Account for borders

        // Calculate content area
        let _inner_area = block.inner(area);

        // Generate diff lines for display
        let diff_lines = self.generate_diff_lines(diff);
        let total_lines = diff_lines.len();

        // Calculate visible range
        let visible_start = self.scroll_position;
        let visible_end = (visible_start + viewport_height).min(total_lines);
        let visible_lines = &diff_lines[visible_start..visible_end];

        // Create text content
        let text = Text::from(visible_lines.to_vec());
        let paragraph = Paragraph::new(text).block(block);

        frame.render_widget(paragraph, area);

        // Render scrollbar if content is scrollable
        if total_lines > viewport_height {
            self.render_scrollbar(frame, area, total_lines, viewport_height);
        }
    }

    fn generate_diff_lines(&self, diff: &Diff) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
            // Determine if this is the current hunk
            let is_current_hunk = self.current_hunk_index == Some(hunk_index);

            // Add hunk header with highlighting if current
            let header_style = if is_current_hunk {
                Style::default().fg(Color::Cyan).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Cyan)
            };

            lines.push(Line::from(Span::styled(hunk.header.clone(), header_style)));

            // Add diff lines
            for diff_line in &hunk.lines {
                let line = self.format_diff_line(diff_line);
                lines.push(line);
            }
        }

        lines
    }

    fn format_diff_line(&self, diff_line: &DiffLine) -> Line<'static> {
        let (prefix, color) = match diff_line.line_type {
            DiffLineType::Addition => ("+", Color::Green),
            DiffLineType::Deletion => ("-", Color::Red),
            DiffLineType::Context => (" ", Color::White),
            DiffLineType::NoNewlineWarning => ("\\", Color::Yellow),
        };

        // Just format the content with prefix, no line numbers for individual lines
        let content = format!("{}{}", prefix, diff_line.content);

        Line::from(Span::styled(content, Style::default().fg(color)))
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
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let max_scroll = self.get_max_scroll_position();
        self.scroll_position = (self.scroll_position + lines).min(max_scroll);
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
    }

    pub fn go_to_bottom(&mut self) {
        let max_scroll = self.get_max_scroll_position();
        self.scroll_position = max_scroll;
    }

    pub fn update_viewport_height(&mut self, area_height: u16) {
        self.viewport_height = area_height.saturating_sub(2) as usize;
    }

    fn get_max_scroll_position(&self) -> usize {
        let total_lines = self.get_total_lines();
        total_lines.saturating_sub(self.viewport_height)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{DiffContext, DiffHunk, DiffLine, DiffLineType};

    fn create_test_diff() -> Diff {
        let hunk = DiffHunk {
            header: "@@ -1,3 +1,4 @@".to_string(),
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 4,
            lines: vec![
                DiffLine {
                    line_type: DiffLineType::Context,
                    content: "line 1".to_string(),
                    old_line_number: Some(1),
                    new_line_number: Some(1),
                },
                DiffLine {
                    line_type: DiffLineType::Deletion,
                    content: "old line".to_string(),
                    old_line_number: Some(2),
                    new_line_number: None,
                },
                DiffLine {
                    line_type: DiffLineType::Addition,
                    content: "new line".to_string(),
                    old_line_number: None,
                    new_line_number: Some(2),
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
        let diff_view = DiffView::new(diff);
        assert_eq!(diff_view.scroll_position, 0);
    }

    #[test]
    fn test_scrolling() {
        let diff = create_test_diff();
        let mut diff_view = DiffView::new(diff);
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
        let diff_view = DiffView::new(diff);
        match &diff_view.content {
            DiffViewContent::Success(diff) => assert!(diff.binary),
            _ => panic!("Expected successful diff content"),
        }
    }

    #[test]
    fn test_hunk_navigation() {
        let diff = create_test_diff();
        let mut diff_view = DiffView::new(diff);
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
        let mut diff_view = DiffView::new(diff);
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
            header: "@@ -1,3 +1,4 @@".to_string(),
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 4,
            lines: vec![
                DiffLine {
                    line_type: DiffLineType::Context,
                    content: "line 1".to_string(),
                    old_line_number: Some(1),
                    new_line_number: Some(1),
                },
                DiffLine {
                    line_type: DiffLineType::Deletion,
                    content: "old line".to_string(),
                    old_line_number: Some(2),
                    new_line_number: None,
                },
            ],
        };

        let hunk2 = DiffHunk {
            header: "@@ -10,2 +11,3 @@".to_string(),
            old_start: 10,
            old_lines: 2,
            new_start: 11,
            new_lines: 3,
            lines: vec![
                DiffLine {
                    line_type: DiffLineType::Context,
                    content: "line 10".to_string(),
                    old_line_number: Some(10),
                    new_line_number: Some(11),
                },
                DiffLine {
                    line_type: DiffLineType::Addition,
                    content: "new line".to_string(),
                    old_line_number: None,
                    new_line_number: Some(12),
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
        let mut diff_view = DiffView::new(diff);
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
    fn test_hunk_count() {
        let single_hunk_diff = create_test_diff();
        let single_hunk_view = DiffView::new(single_hunk_diff);
        assert_eq!(single_hunk_view.get_hunk_count(), 1);

        let multi_hunk_diff = create_multi_hunk_diff();
        let multi_hunk_view = DiffView::new(multi_hunk_diff);
        assert_eq!(multi_hunk_view.get_hunk_count(), 2);

        let empty_diff = Diff {
            file_path: "empty.txt".to_string(),
            context: DiffContext::WorkingTreeToIndex,
            hunks: vec![],
            binary: false,
        };
        let empty_view = DiffView::new(empty_diff);
        assert_eq!(empty_view.get_hunk_count(), 0);
        assert_eq!(empty_view.get_current_hunk_index(), None);
    }
}
