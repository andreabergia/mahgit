use crate::config::Config;
use crate::diff::{Diff, DiffLine, LineType, syntax_highlighter};
use crate::ui::inline_diff::InlineDiffRenderer;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use syntect::{easy::HighlightLines, parsing::SyntaxReference};

#[derive(Debug, Clone, Copy)]
pub struct LineNumberWidths {
    pub old: usize,
    pub new: usize,
}

pub struct DiffRenderer<'a> {
    config: &'a Config,
}

/// Context for rendering a single diff with syntax highlighting state
pub struct DiffRenderContext<'a> {
    renderer: &'a DiffRenderer<'a>,
    line_number_widths: Option<LineNumberWidths>,
    syntax_ref: Option<&'static SyntaxReference>,
}

impl<'a> DiffRenderContext<'a> {
    pub fn new(renderer: &'a DiffRenderer<'a>, diff: &Diff) -> Self {
        let line_number_widths = if renderer.config.show_line_numbers {
            Some(DiffRenderer::calculate_line_number_widths(diff))
        } else {
            None
        };

        let syntax_highlighter = syntax_highlighter();
        let syntax_ref = syntax_highlighter.detect_syntax(&diff.file_path);

        Self {
            renderer,
            line_number_widths,
            syntax_ref,
        }
    }

    /// Create a fresh highlighter instance for a hunk.
    /// This prevents syntax highlighting state from leaking between hunks.
    pub fn create_fresh_highlighter(&self) -> Option<HighlightLines<'static>> {
        self.syntax_ref.map(|syntax| {
            let syntax_highlighter = syntax_highlighter();
            syntax_highlighter.create_highlighter(syntax)
        })
    }

    pub fn format_diff_line(
        &self,
        diff_line: &DiffLine,
        is_active_hunk: bool,
        prefix: Option<&str>,
        highlighter: Option<&mut HighlightLines<'static>>,
    ) -> Line<'static> {
        let syntax_state = if let (Some(syntax), Some(hl)) = (self.syntax_ref, highlighter) {
            Some((syntax, hl))
        } else {
            None
        };

        self.renderer.format_diff_line(
            diff_line,
            self.line_number_widths.as_ref(),
            is_active_hunk,
            prefix,
            syntax_state,
        )
    }
}

impl<'a> DiffRenderer<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    /// Calculate the maximum line number widths for formatting
    pub fn calculate_line_number_widths(diff: &Diff) -> LineNumberWidths {
        let mut max_old = 0;
        let mut max_new = 0;

        for hunk in &diff.hunks {
            for line in &hunk.lines {
                if let Some(old) = line.old_line_no {
                    max_old = max_old.max(old);
                }
                if let Some(new) = line.new_line_no {
                    max_new = max_new.max(new);
                }
            }
        }

        LineNumberWidths {
            old: Self::digit_width(max_old),
            new: Self::digit_width(max_new),
        }
    }

    /// Calculate the number of digits needed to display a number
    fn digit_width(mut value: usize) -> usize {
        let mut width = 1;
        while value >= 10 {
            value /= 10;
            width += 1;
        }
        width
    }

    /// Format a line number with proper width and alignment
    pub fn format_line_number(number: Option<usize>, width: usize) -> String {
        let text = number.map(|n| n.to_string()).unwrap_or_default();
        format!("{text:>width$}")
    }

    /// Format a single diff line with optional line numbers and highlighting
    pub fn format_diff_line(
        &self,
        diff_line: &DiffLine,
        line_number_widths: Option<&LineNumberWidths>,
        is_active_hunk: bool,
        prefix: Option<&str>,
        syntax_state: Option<(&SyntaxReference, &mut HighlightLines)>,
    ) -> Line<'static> {
        let (line_prefix, color) = match diff_line.line_type {
            LineType::Addition => ("+", self.config.theme.staged),
            LineType::Deletion => ("-", self.config.theme.unstaged),
            LineType::Context => (" ", self.config.theme.diff_context),
            LineType::NoNewlineEOF => ("\\", self.config.theme.diff_no_newline),
        };

        let gutter_color = match diff_line.line_type {
            LineType::Addition => self.config.theme.diff_gutter_addition,
            LineType::Deletion => self.config.theme.diff_gutter_deletion,
            LineType::Context => self.config.theme.diff_gutter_context,
            LineType::NoNewlineEOF => self.config.theme.diff_no_newline,
        };

        // Create base style for content - will be used differently depending on whether
        // we have syntax highlighting or not
        // Always prefer syntax highlighting over inline diffs when available
        let has_syntax = syntax_state.is_some();

        // For lines with inline diff, use default foreground (to avoid color-on-color).
        // For lines without inline diff:
        //   - If we have syntax highlighting, use default foreground (syntax will provide colors)
        //   - If no syntax highlighting, use the theme color
        let mut content_style = if diff_line.inline_diff.is_some() || has_syntax {
            Style::default()
        } else {
            Style::default().fg(color)
        };

        // Create a separate style for the line prefix that always has the diff color
        let mut prefix_style = Style::default().fg(color);

        let mut gutter_style = Style::default().fg(gutter_color);
        if matches!(diff_line.line_type, LineType::Context) && !is_active_hunk {
            content_style = content_style.add_modifier(Modifier::DIM);
            prefix_style = prefix_style.add_modifier(Modifier::DIM);
            gutter_style = gutter_style.add_modifier(Modifier::DIM);
        }

        if is_active_hunk {
            gutter_style = gutter_style.bg(self.config.theme.diff_gutter_focused);
        }

        // Only apply hunk highlighting if we don't have syntax highlighting
        // (syntax colors need default background for proper contrast)
        if !has_syntax {
            content_style = self.apply_hunk_highlight(content_style, is_active_hunk);
        }
        prefix_style = self.apply_hunk_highlight(prefix_style, is_active_hunk);
        gutter_style = self.apply_hunk_highlight(gutter_style, is_active_hunk);

        // Expand tabs in the line content
        let expanded_content =
            crate::config::expand_tabs_with_width(&diff_line.content, self.config.tab_width);

        let mut spans: Vec<Span> = Vec::new();

        // Add optional prefix (for indentation in inline mode)
        if let Some(indent) = prefix {
            spans.push(Span::raw(indent.to_string()));
        }

        if let Some(widths) = line_number_widths {
            let mut line_number_style = Style::default().fg(self.config.theme.diff_line_number);
            if matches!(diff_line.line_type, LineType::Context) && !is_active_hunk {
                line_number_style = line_number_style.add_modifier(Modifier::DIM);
            }
            line_number_style = self.apply_hunk_highlight(line_number_style, is_active_hunk);

            spans.push(Span::styled(
                Self::format_line_number(diff_line.old_line_no, widths.old),
                line_number_style,
            ));
            spans.push(Span::styled(" ", line_number_style));
            spans.push(Span::styled(
                Self::format_line_number(diff_line.new_line_no, widths.new),
                line_number_style,
            ));
            spans.push(Span::styled(" ", line_number_style));
        }

        spans.push(Span::styled("|", gutter_style));
        spans.push(Span::styled(
            " ",
            self.apply_hunk_highlight(Style::default(), is_active_hunk),
        ));

        let content_spans = if let Some((syntax_ref, highlighter)) = syntax_state {
            // Prefer syntax highlighting over inline diffs when available
            let syntax_highlighter = syntax_highlighter();
            let highlighted =
                syntax_highlighter.highlight_line(&expanded_content, syntax_ref, highlighter);

            // Use prefix_style for the line prefix to show diff color (green/red)
            let mut result = vec![Span::styled(line_prefix.to_string(), prefix_style)];

            // Determine background color based on line type
            let bg_color = match diff_line.line_type {
                LineType::Addition => Some(self.config.theme.diff_inline_addition),
                LineType::Deletion => Some(self.config.theme.diff_inline_deletion),
                LineType::Context => None,
                LineType::NoNewlineEOF => None,
            };

            // Apply syntax colors to foreground AND diff backgrounds to content
            for (text, syntax_color) in highlighted {
                let mut style = Style::default().fg(syntax_color);
                if let Some(bg) = bg_color {
                    style = style.bg(bg);
                }
                result.push(Span::styled(text, style));
            }

            result
        } else if let Some(inline_diff) = &diff_line.inline_diff {
            // Fall back to inline diffs for word-level change highlighting
            let renderer = InlineDiffRenderer::new(self.config);
            renderer.inline_content_spans(inline_diff, content_style, line_prefix)
        } else {
            // No syntax highlighting or inline diff - plain styling
            vec![Span::styled(
                format!("{}{}", line_prefix, expanded_content),
                content_style,
            )]
        };

        spans.extend(content_spans);

        Line::from(spans)
    }

    /// Generate all diff lines for a diff, with optional hunk highlighting
    pub fn generate_diff_lines(
        &self,
        diff: &Diff,
        active_hunk_index: Option<usize>,
        prefix: Option<&str>,
    ) -> Vec<Line<'static>> {
        let context = DiffRenderContext::new(self, diff);
        let mut lines = Vec::new();

        for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
            // Determine if this is the current hunk
            let is_current_hunk = active_hunk_index == Some(hunk_index);

            // Add hunk header
            let header_style = self.config.theme.diff_hunk_header;

            let header_line = if let Some(indent) = prefix {
                Line::from(vec![
                    Span::raw(indent.to_string()),
                    Span::styled(hunk.header.raw.clone(), header_style),
                ])
            } else {
                Line::from(Span::styled(hunk.header.raw.clone(), header_style))
            };

            lines.push(header_line);

            // Create a fresh highlighter for this hunk to prevent state leakage
            let mut highlighter = context.create_fresh_highlighter();

            // Add diff lines
            for diff_line in &hunk.lines {
                let line = context.format_diff_line(
                    diff_line,
                    is_current_hunk,
                    prefix,
                    highlighter.as_mut(),
                );
                lines.push(line);
            }
        }

        lines
    }

    fn apply_hunk_highlight(&self, style: Style, is_current_hunk: bool) -> Style {
        if is_current_hunk {
            if style.bg.is_some() {
                style
            } else {
                style.bg(self.config.theme.diff_hunk_highlight)
            }
        } else {
            style
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{DiffContext, DiffHunk, HunkHeader, LineRange};

    fn test_config() -> Config {
        Config {
            theme: crate::theme::Theme::default(),
            tab_width: 4,
            show_line_numbers: true,
            word_wrap: false,
            ignore_whitespace: false,
        }
    }

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
    fn test_line_number_width_calculation() {
        let diff = create_test_diff();
        let widths = DiffRenderer::calculate_line_number_widths(&diff);
        assert_eq!(widths.old, 1); // max old line is 2, which is 1 digit
        assert_eq!(widths.new, 1); // max new line is 2, which is 1 digit
    }

    #[test]
    fn test_format_line_number() {
        assert_eq!(DiffRenderer::format_line_number(Some(1), 3), "  1");
        assert_eq!(DiffRenderer::format_line_number(Some(10), 3), " 10");
        assert_eq!(DiffRenderer::format_line_number(Some(100), 3), "100");
        assert_eq!(DiffRenderer::format_line_number(None, 3), "   ");
    }

    #[test]
    fn test_format_diff_line_with_line_numbers() {
        let config = test_config();
        let renderer = DiffRenderer::new(&config);
        let diff = create_test_diff();
        let widths = DiffRenderer::calculate_line_number_widths(&diff);

        let line =
            renderer.format_diff_line(&diff.hunks[0].lines[0], Some(&widths), false, None, None);

        // Should have: old_line, space, new_line, space, gutter, space, content
        assert!(line.spans.len() >= 7);
        assert_eq!(line.spans[0].content, "1");
        assert_eq!(line.spans[2].content, "1");
        assert_eq!(line.spans[4].content, "|");
    }

    #[test]
    fn test_format_diff_line_without_line_numbers() {
        let mut config = test_config();
        config.show_line_numbers = false;
        let renderer = DiffRenderer::new(&config);
        let diff = create_test_diff();

        let line = renderer.format_diff_line(&diff.hunks[0].lines[0], None, false, None, None);

        // Should have: gutter, space, content (no line numbers)
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content, "|");
    }

    #[test]
    fn test_format_diff_line_with_prefix() {
        let config = test_config();
        let renderer = DiffRenderer::new(&config);
        let diff = create_test_diff();

        let line =
            renderer.format_diff_line(&diff.hunks[0].lines[0], None, false, Some("    "), None);

        // First span should be the prefix
        assert_eq!(line.spans[0].content, "    ");
        assert_eq!(line.spans[1].content, "|");
    }

    #[test]
    fn test_generate_diff_lines() {
        let config = test_config();
        let renderer = DiffRenderer::new(&config);
        let diff = create_test_diff();

        let lines = renderer.generate_diff_lines(&diff, None, None);

        // Should have: 1 header + 3 diff lines = 4 lines total
        assert_eq!(lines.len(), 4);

        // First line should be the header
        assert!(lines[0].spans.iter().any(|s| s.content.contains("@@")));
    }

    #[test]
    fn test_hunk_highlighting() {
        let config = test_config();
        let renderer = DiffRenderer::new(&config);
        let diff = create_test_diff();
        let widths = DiffRenderer::calculate_line_number_widths(&diff);

        // Without highlighting
        let line_no_highlight =
            renderer.format_diff_line(&diff.hunks[0].lines[0], Some(&widths), false, None, None);
        assert_eq!(line_no_highlight.spans[0].style.bg, None);

        // With highlighting
        let line_with_highlight =
            renderer.format_diff_line(&diff.hunks[0].lines[0], Some(&widths), true, None, None);
        assert_eq!(
            line_with_highlight.spans[0].style.bg,
            Some(config.theme.diff_hunk_highlight)
        );
    }

    /// Test that demonstrates the syntax highlighting state corruption bug.
    ///
    /// This test creates a diff with two hunks separated by a gap in line numbers.
    /// Hunk 1 ends with a line that opens a multi-line comment.
    /// Hunk 2 (30 lines later) has code that should NOT be inside a comment.
    ///
    /// CURRENT BUG: The HighlightLines state from hunk 1 carries over to hunk 2,
    /// causing hunk 2's code to be incorrectly highlighted as if it's inside the comment.
    #[test]
    fn test_syntax_highlighting_state_corruption_across_hunks() {
        let config = test_config();
        let renderer = DiffRenderer::new(&config);

        // Create a diff for a Rust file with two hunks separated by a gap
        let diff = Diff {
            file_path: "test.rs".to_string(),
            context: DiffContext::WorkingTreeToIndex,
            binary: false,
            hunks: vec![
                // Hunk 1: Lines 10-15, ends with opening a multi-line comment
                DiffHunk {
                    header: HunkHeader {
                        raw: "@@ -10,3 +10,3 @@".to_string(),
                        old_start: 10,
                        old_lines: 3,
                        new_start: 10,
                        new_lines: 3,
                    },
                    old_range: LineRange {
                        start: 10,
                        count: 3,
                    },
                    new_range: LineRange {
                        start: 10,
                        count: 3,
                    },
                    stageable: true,
                    context_lines: 3,
                    lines: vec![
                        DiffLine {
                            content: "fn foo() {".to_string(),
                            line_type: LineType::Context,
                            old_line_no: Some(10),
                            new_line_no: Some(10),
                            inline_diff: None,
                        },
                        DiffLine {
                            content: "    let x = 5;".to_string(),
                            line_type: LineType::Deletion,
                            old_line_no: Some(11),
                            new_line_no: None,
                            inline_diff: None,
                        },
                        DiffLine {
                            content: "    /* Start of a comment".to_string(),
                            line_type: LineType::Addition,
                            old_line_no: None,
                            new_line_no: Some(11),
                            inline_diff: None,
                        },
                        DiffLine {
                            content: "}".to_string(),
                            line_type: LineType::Context,
                            old_line_no: Some(12),
                            new_line_no: Some(12),
                            inline_diff: None,
                        },
                    ],
                },
                // Hunk 2: Lines 50-52 (30 lines later, gap in between)
                // This code is OUTSIDE the multi-line comment in the actual file
                DiffHunk {
                    header: HunkHeader {
                        raw: "@@ -50,3 +50,3 @@".to_string(),
                        old_start: 50,
                        old_lines: 3,
                        new_start: 50,
                        new_lines: 3,
                    },
                    old_range: LineRange {
                        start: 50,
                        count: 3,
                    },
                    new_range: LineRange {
                        start: 50,
                        count: 3,
                    },
                    stageable: true,
                    context_lines: 3,
                    lines: vec![
                        DiffLine {
                            content: "fn bar() {".to_string(),
                            line_type: LineType::Context,
                            old_line_no: Some(50),
                            new_line_no: Some(50),
                            inline_diff: None,
                        },
                        DiffLine {
                            content: "    let y = 10;".to_string(),
                            line_type: LineType::Deletion,
                            old_line_no: Some(51),
                            new_line_no: None,
                            inline_diff: None,
                        },
                        DiffLine {
                            content: "    let y = 20;".to_string(),
                            line_type: LineType::Addition,
                            old_line_no: None,
                            new_line_no: Some(51),
                            inline_diff: None,
                        },
                        DiffLine {
                            content: "}".to_string(),
                            line_type: LineType::Context,
                            old_line_no: Some(52),
                            new_line_no: Some(52),
                            inline_diff: None,
                        },
                    ],
                },
            ],
        };

        // Render the diff lines with syntax highlighting
        let lines = renderer.generate_diff_lines(&diff, None, None);

        // Expected structure:
        // Line 0: Hunk 1 header "@@ -10,3 +10,3 @@"
        // Lines 1-4: Hunk 1 content (4 lines)
        // Line 5: Hunk 2 header "@@ -50,3 +50,3 @@"
        // Lines 6-9: Hunk 2 content (4 lines)
        assert_eq!(lines.len(), 10);

        // Find the line that says "fn bar() {" in hunk 2
        // This is line 6 (after hunk 2 header at line 5)
        let bar_function_line = &lines[6];

        // Get the syntax highlighter to compare expected vs actual highlighting
        let syntax_highlighter = syntax_highlighter();
        let syntax = syntax_highlighter
            .detect_syntax("test.rs")
            .expect("Should detect Rust syntax");

        // Create a FRESH highlighter to see what the CORRECT highlighting should be
        // (as if we're highlighting this line in isolation)
        let mut fresh_highlighter = syntax_highlighter.create_highlighter(syntax);
        let expected_spans =
            syntax_highlighter.highlight_line("fn bar() {", syntax, &mut fresh_highlighter);

        // The "fn" keyword should be highlighted with a specific color (not comment color)
        // Let's find the span containing "fn" in the expected output
        let expected_fn_span = expected_spans
            .iter()
            .find(|(text, _color)| text.contains("fn"))
            .expect("Should find 'fn' keyword in expected highlighting");

        // Now check what color was actually used in the rendered line
        // Find the span containing "fn" in the actual rendered line
        let actual_fn_span = bar_function_line
            .spans
            .iter()
            .find(|span| span.content.contains("fn"))
            .expect("Should find 'fn' keyword in rendered line");

        // BUG DEMONSTRATION:
        // With the current implementation, if syntax highlighting is enabled,
        // the "fn" keyword in hunk 2 will be highlighted with the WRONG color
        // because the highlighter still thinks it's inside the comment from hunk 1.
        //
        // After the fix, these colors should match (both should be the keyword color,
        // not the comment color).

        println!("Expected 'fn' color: {:?}", expected_fn_span.1);
        println!("Actual 'fn' color: {:?}", actual_fn_span.style.fg);

        // This assertion will fail with the current buggy implementation
        // (the colors won't match because of state corruption)
        assert_eq!(
            actual_fn_span.style.fg,
            Some(expected_fn_span.1),
            "The 'fn' keyword in hunk 2 should be highlighted as a keyword, \
             not as a comment. State from hunk 1 is leaking into hunk 2!"
        );
    }
}
