use crate::{
    config::Config,
    diff::{InlineChange, InlineDiff},
};
use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

pub struct InlineDiffRenderer<'a> {
    config: &'a Config,
}

impl<'a> InlineDiffRenderer<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    pub fn inline_content_spans(
        &self,
        inline_diff: &InlineDiff,
        base_style: Style,
        prefix: &str,
    ) -> Vec<Span<'static>> {
        let mut spans = Vec::with_capacity(inline_diff.segments.len() + 1);
        spans.push(Span::styled(prefix.to_string(), base_style));

        let mut col = 0usize;
        for segment in &inline_diff.segments {
            let expanded = self.expand_segment_with_tabs(&segment.content, &mut col);
            let mut style = match segment.change {
                InlineChange::Unchanged => base_style,
                InlineChange::Added => base_style.bg(self.config.theme.diff_inline_addition),
                InlineChange::Removed => base_style.bg(self.config.theme.diff_inline_deletion),
            };
            if matches!(segment.change, InlineChange::Added | InlineChange::Removed) {
                style = style.add_modifier(Modifier::BOLD);
            }
            spans.push(Span::styled(expanded, style));
        }

        spans
    }

    fn expand_segment_with_tabs(&self, segment: &str, col: &mut usize) -> String {
        let mut result = String::with_capacity(segment.len());
        for ch in segment.chars() {
            if ch == '\t' {
                let spaces = self.config.tab_width - (*col % self.config.tab_width);
                result.extend(std::iter::repeat_n(' ', spaces));
                *col += spaces;
            } else {
                result.push(ch);
                *col += 1;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, diff::InlineDiffSegment, theme::Theme};
    use ratatui::style::Modifier;

    fn test_config() -> Config {
        Config {
            theme: Theme::from_name("gruvbox-dark").unwrap(),
            tab_width: 4,
            show_line_numbers: true,
        }
    }

    #[test]
    fn renderer_expands_tabs_and_applies_styles() {
        let config = test_config();
        let renderer = InlineDiffRenderer::new(&config);
        let inline_diff = InlineDiff {
            segments: vec![
                InlineDiffSegment {
                    content: "ab\t".to_string(),
                    change: InlineChange::Unchanged,
                },
                InlineDiffSegment {
                    content: "X".to_string(),
                    change: InlineChange::Added,
                },
                InlineDiffSegment {
                    content: "Y".to_string(),
                    change: InlineChange::Removed,
                },
            ],
        };

        let spans = renderer.inline_content_spans(&inline_diff, Style::default(), "+");

        assert_eq!(spans[0].content, "+");
        assert_eq!(spans[1].content, "ab  "); // tab expands to next multiple of 4
        assert_eq!(spans[2].content, "X");
        assert_eq!(spans[3].content, "Y");

        assert_eq!(spans[2].style.bg, Some(config.theme.diff_inline_addition));
        assert!(spans[2].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(spans[3].style.bg, Some(config.theme.diff_inline_deletion));
        assert!(spans[3].style.add_modifier.contains(Modifier::BOLD));
        assert!(spans[1].style.bg.is_none());
    }

    #[test]
    fn renderer_keeps_prefix_separate_from_content() {
        let config = test_config();
        let renderer = InlineDiffRenderer::new(&config);
        let inline_diff = InlineDiff {
            segments: vec![InlineDiffSegment {
                content: "line".to_string(),
                change: InlineChange::Added,
            }],
        };

        let spans = renderer.inline_content_spans(&inline_diff, Style::default(), "-");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "-");
        assert_eq!(spans[1].content, "line");
    }
}
