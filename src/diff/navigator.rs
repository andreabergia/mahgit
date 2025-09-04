use crate::diff::{Diff, HunkPosition};

#[derive(Debug)]
pub struct HunkNavigator {
    pub current_hunk: usize,
    pub total_hunks: usize,
    pub hunk_positions: Vec<HunkPosition>,
    pub scroll_offset: usize,
}

impl HunkNavigator {
    pub fn new(diff: &Diff) -> Self {
        let total_hunks = diff.hunks.len();
        let hunk_positions = Self::calculate_hunk_positions(diff);

        Self {
            current_hunk: 0,
            total_hunks,
            hunk_positions,
            scroll_offset: 0,
        }
    }

    /// Calculate screen positions for each hunk in the diff
    fn calculate_hunk_positions(diff: &Diff) -> Vec<HunkPosition> {
        let mut positions = Vec::new();
        let mut current_line = 0;

        for hunk in &diff.hunks {
            let start_line = current_line;
            // Each hunk has: 1 header line + all diff lines
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

    /// Navigate to the next hunk, wrapping around if at the end
    pub fn next_hunk(&mut self) -> Option<usize> {
        if self.total_hunks == 0 {
            return None;
        }

        self.current_hunk = (self.current_hunk + 1) % self.total_hunks;
        Some(self.current_hunk)
    }

    /// Navigate to the previous hunk, wrapping around if at the beginning
    pub fn previous_hunk(&mut self) -> Option<usize> {
        if self.total_hunks == 0 {
            return None;
        }

        self.current_hunk = if self.current_hunk == 0 {
            self.total_hunks - 1
        } else {
            self.current_hunk - 1
        };
        Some(self.current_hunk)
    }

    /// Get the screen position for the current hunk
    pub fn get_current_hunk_position(&self) -> Option<&HunkPosition> {
        self.hunk_positions.get(self.current_hunk)
    }

    /// Get the screen position for a specific hunk
    pub fn get_hunk_position(&self, hunk_index: usize) -> Option<&HunkPosition> {
        self.hunk_positions.get(hunk_index)
    }

    /// Update scroll offset to keep the current hunk visible
    pub fn update_scroll_for_current_hunk(&mut self, viewport_height: usize) {
        if let Some(position) = self.get_current_hunk_position() {
            // Check if current hunk is visible within the viewport
            let viewport_start = self.scroll_offset;
            let viewport_end = viewport_start + viewport_height;

            // If hunk starts before viewport, scroll up to show it
            if position.start_line < viewport_start {
                self.scroll_offset = position.start_line;
            }
            // If hunk ends after viewport, scroll down to show it
            else if position.end_line > viewport_end {
                // Try to position the hunk at the bottom of the viewport
                self.scroll_offset = position.end_line.saturating_sub(viewport_height);
            }
        }
    }

    /// Jump directly to a specific hunk
    pub fn jump_to_hunk(&mut self, hunk_index: usize, viewport_height: usize) -> bool {
        if hunk_index >= self.total_hunks {
            return false;
        }

        self.current_hunk = hunk_index;
        self.update_scroll_for_current_hunk(viewport_height);
        true
    }

    /// Get current hunk index
    pub fn current_hunk_index(&self) -> usize {
        self.current_hunk
    }

    /// Check if there are any hunks to navigate
    pub fn has_hunks(&self) -> bool {
        self.total_hunks > 0
    }

    /// Get total number of hunks
    pub fn hunk_count(&self) -> usize {
        self.total_hunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{DiffContext, DiffHunk, DiffLine, HunkHeader, LineRange, LineType};

    fn create_test_diff_with_multiple_hunks() -> Diff {
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
                },
                DiffLine {
                    content: "old line".to_string(),
                    line_type: LineType::Deletion,
                    old_line_no: Some(2),
                    new_line_no: None,
                },
                DiffLine {
                    content: "new line".to_string(),
                    line_type: LineType::Addition,
                    old_line_no: None,
                    new_line_no: Some(2),
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
                },
                DiffLine {
                    content: "new line added".to_string(),
                    line_type: LineType::Addition,
                    old_line_no: None,
                    new_line_no: Some(12),
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
    fn test_navigator_creation() {
        let diff = create_test_diff_with_multiple_hunks();
        let navigator = HunkNavigator::new(&diff);

        assert_eq!(navigator.current_hunk, 0);
        assert_eq!(navigator.total_hunks, 2);
        assert_eq!(navigator.hunk_positions.len(), 2);
        assert_eq!(navigator.scroll_offset, 0);
    }

    #[test]
    fn test_hunk_position_calculation() {
        let diff = create_test_diff_with_multiple_hunks();
        let navigator = HunkNavigator::new(&diff);

        // First hunk: header (1) + 3 lines = 4 total lines, starts at 0
        let pos1 = &navigator.hunk_positions[0];
        assert_eq!(pos1.start_line, 0);
        assert_eq!(pos1.end_line, 4); // 1 header + 3 lines
        assert_eq!(pos1.screen_y, 0);

        // Second hunk: starts where first ended (4), has 1 header + 2 lines = 3 total
        let pos2 = &navigator.hunk_positions[1];
        assert_eq!(pos2.start_line, 4);
        assert_eq!(pos2.end_line, 7); // 4 + 1 header + 2 lines
        assert_eq!(pos2.screen_y, 4);
    }

    #[test]
    fn test_hunk_navigation() {
        let diff = create_test_diff_with_multiple_hunks();
        let mut navigator = HunkNavigator::new(&diff);

        // Should start at hunk 0
        assert_eq!(navigator.current_hunk_index(), 0);

        // Navigate to next hunk
        let next = navigator.next_hunk();
        assert_eq!(next, Some(1));
        assert_eq!(navigator.current_hunk_index(), 1);

        // Navigate to next hunk (should wrap around)
        let next = navigator.next_hunk();
        assert_eq!(next, Some(0));
        assert_eq!(navigator.current_hunk_index(), 0);

        // Navigate to previous hunk (should wrap around)
        let prev = navigator.previous_hunk();
        assert_eq!(prev, Some(1));
        assert_eq!(navigator.current_hunk_index(), 1);

        // Navigate to previous hunk
        let prev = navigator.previous_hunk();
        assert_eq!(prev, Some(0));
        assert_eq!(navigator.current_hunk_index(), 0);
    }

    #[test]
    fn test_empty_diff_navigation() {
        let empty_diff = Diff {
            file_path: "empty.txt".to_string(),
            context: DiffContext::WorkingTreeToIndex,
            hunks: vec![],
            binary: false,
        };

        let mut navigator = HunkNavigator::new(&empty_diff);

        assert_eq!(navigator.total_hunks, 0);
        assert_eq!(navigator.hunk_count(), 0);
        assert!(!navigator.has_hunks());

        // Navigation should return None for empty diffs
        assert_eq!(navigator.next_hunk(), None);
        assert_eq!(navigator.previous_hunk(), None);
        assert_eq!(navigator.get_current_hunk_position(), None);
    }

    #[test]
    fn test_jump_to_hunk() {
        let diff = create_test_diff_with_multiple_hunks();
        let mut navigator = HunkNavigator::new(&diff);
        let viewport_height = 10;

        // Jump to hunk 1
        let result = navigator.jump_to_hunk(1, viewport_height);
        assert!(result);
        assert_eq!(navigator.current_hunk_index(), 1);

        // Jump to invalid hunk
        let result = navigator.jump_to_hunk(5, viewport_height);
        assert!(!result);
        // Should stay at current hunk
        assert_eq!(navigator.current_hunk_index(), 1);

        // Jump back to hunk 0
        let result = navigator.jump_to_hunk(0, viewport_height);
        assert!(result);
        assert_eq!(navigator.current_hunk_index(), 0);
    }

    #[test]
    fn test_scroll_update_for_current_hunk() {
        let diff = create_test_diff_with_multiple_hunks();
        let mut navigator = HunkNavigator::new(&diff);
        let viewport_height = 5;

        // Start at hunk 0, scroll should be 0
        navigator.update_scroll_for_current_hunk(viewport_height);
        assert_eq!(navigator.scroll_offset, 0);

        // Jump to hunk 1 (starts at line 4)
        navigator.jump_to_hunk(1, viewport_height);

        // Since hunk 1 has end_line = 7 and viewport_height = 5,
        // the scroll should be 7 - 5 = 2 to fit the hunk at the bottom of viewport
        assert_eq!(navigator.scroll_offset, 2);
    }
}
