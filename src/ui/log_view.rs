use crate::config::Config;
use crate::log::LogData;
use crate::repository::CommitChangeType;
use crate::ui::diff_renderer::DiffRenderer;
use crate::ui::log_navigation::{LogCursor, LogNavigationState};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

pub struct LogView<'a> {
    log_data: &'a LogData,
    navigation: &'a LogNavigationState,
    config: &'a Config,
}

impl<'a> LogView<'a> {
    pub fn new(
        log_data: &'a LogData,
        navigation: &'a LogNavigationState,
        config: &'a Config,
    ) -> Self {
        Self {
            log_data,
            navigation,
            config,
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .constraints([Constraint::Min(3)].as_ref())
            .split(area);

        let title = format!(
            "Log: {} ({} commits)",
            self.log_data.branch_name,
            self.log_data.entries.len()
        );

        let block = Block::default().title(title).borders(Borders::ALL);
        let inner_area = block.inner(chunks[0]);
        f.render_widget(block, chunks[0]);

        let content_area = inner_area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });

        if self.log_data.entries.is_empty() {
            return;
        }

        let mut items = Vec::new();
        let mut selected_list_index = None;
        let cursor = self.navigation.current_cursor();

        for (commit_idx, entry) in self.log_data.entries.iter().enumerate() {
            let is_selected = matches!(
                cursor,
                Some(LogCursor::Commit { index }) if index == commit_idx
            );

            if is_selected {
                selected_list_index = Some(items.len());
            }

            let expanded = self.navigation.is_commit_expanded(commit_idx);
            let expand_icon = if expanded { "▾" } else { "▸" };

            let line = self.format_commit_line(entry, is_selected, expand_icon);
            items.push(ListItem::new(line));

            // If commit is expanded, show changed files
            if expanded && let Some(expansion) = self.navigation.get_expansion(commit_idx) {
                for (file_idx, file_change) in expansion.files.iter().enumerate() {
                    let is_file_selected = matches!(
                        cursor,
                        Some(LogCursor::File { commit_index, file_index })
                        if commit_index == commit_idx && file_index == file_idx
                    );

                    if is_file_selected {
                        selected_list_index = Some(items.len());
                    }

                    let file_expanded = self.navigation.is_file_expanded(commit_idx, file_idx);
                    let file_line =
                        self.format_file_line(file_change, is_file_selected, file_expanded);
                    items.push(ListItem::new(file_line));

                    // If file is expanded, show diff hunks
                    if file_expanded && let Some(diff) = expansion.file_diffs.get(&file_idx) {
                        self.add_inline_diff_items(
                            &mut items,
                            diff,
                            &mut selected_list_index,
                            cursor,
                            commit_idx,
                            file_idx,
                        );
                    }
                }
            }
        }

        // Show load-more indicator
        if self.log_data.has_more {
            items.push(ListItem::new(Line::from(Span::styled(
                "  ... (scroll down to load more)",
                Style::default().fg(self.config.theme.log_date),
            ))));
        }

        let viewport_height = content_area.height as usize;
        self.navigation
            .update_viewport_metrics(viewport_height, items.len());

        let list = List::new(items);
        let mut list_state = ratatui::widgets::ListState::default();
        if !self.navigation.is_manual_scroll_active() {
            list_state.select(selected_list_index);
        }
        *list_state.offset_mut() = self.navigation.scroll_offset();

        f.render_stateful_widget(list, content_area, &mut list_state);
    }

    fn format_commit_line(
        &self,
        entry: &crate::log::LogEntry,
        is_selected: bool,
        expand_icon: &str,
    ) -> Line<'static> {
        let relative_date = format_relative_time(&entry.time);
        let theme = &self.config.theme;

        let selected_style = Style::default()
            .bg(theme.selected_bg)
            .fg(theme.selected_fg)
            .add_modifier(Modifier::BOLD);

        let prefix = if is_selected { "> " } else { "  " };

        let mut spans = Vec::new();

        spans.push(Span::styled(
            prefix.to_string(),
            if is_selected {
                selected_style
            } else {
                Style::default()
            },
        ));

        spans.push(Span::styled(
            format!("{} ", expand_icon),
            if is_selected {
                selected_style
            } else {
                Style::default().fg(theme.log_date)
            },
        ));

        // Hash
        spans.push(Span::styled(
            entry.short_hash.clone(),
            if is_selected {
                selected_style
            } else {
                Style::default().fg(theme.log_hash)
            },
        ));
        spans.push(Span::styled(
            " ".to_string(),
            if is_selected {
                selected_style
            } else {
                Style::default()
            },
        ));

        // Summary
        spans.push(Span::styled(
            entry.summary.clone(),
            if is_selected {
                selected_style
            } else {
                Style::default().fg(theme.diff_context)
            },
        ));
        spans.push(Span::styled(
            " ".to_string(),
            if is_selected {
                selected_style
            } else {
                Style::default()
            },
        ));

        // Author
        spans.push(Span::styled(
            entry.author_name.clone(),
            if is_selected {
                selected_style
            } else {
                Style::default().fg(theme.log_author)
            },
        ));
        spans.push(Span::styled(
            " ".to_string(),
            if is_selected {
                selected_style
            } else {
                Style::default()
            },
        ));

        // Relative date
        spans.push(Span::styled(
            relative_date,
            if is_selected {
                selected_style
            } else {
                Style::default().fg(theme.log_date)
            },
        ));

        Line::from(spans)
    }

    fn format_file_line(
        &self,
        file_change: &crate::repository::CommitFileChange,
        is_selected: bool,
        is_expanded: bool,
    ) -> Line<'static> {
        let theme = &self.config.theme;

        let selected_style = Style::default()
            .bg(theme.selected_bg)
            .fg(theme.selected_fg)
            .add_modifier(Modifier::BOLD);

        let change_color = match file_change.change_type {
            CommitChangeType::Added => theme.staged,
            CommitChangeType::Deleted => theme.unstaged,
            CommitChangeType::Modified => theme.diff_context,
            CommitChangeType::Renamed => theme.untracked,
            CommitChangeType::Other => theme.diff_context,
        };

        let prefix = if is_selected { "  > " } else { "    " };
        let expand_icon = if is_expanded { "▾" } else { "▸" };

        let mut spans = Vec::new();
        spans.push(Span::styled(
            prefix.to_string(),
            if is_selected {
                selected_style
            } else {
                Style::default()
            },
        ));
        spans.push(Span::styled(
            format!("{} ", expand_icon),
            if is_selected {
                selected_style
            } else {
                Style::default().fg(theme.log_date)
            },
        ));
        spans.push(Span::styled(
            format!("{} ", file_change.change_type),
            if is_selected {
                selected_style
            } else {
                Style::default().fg(change_color)
            },
        ));
        spans.push(Span::styled(
            file_change.path.clone(),
            if is_selected {
                selected_style
            } else {
                Style::default().fg(theme.diff_context)
            },
        ));

        Line::from(spans)
    }

    fn add_inline_diff_items(
        &self,
        items: &mut Vec<ListItem>,
        diff: &crate::diff::Diff,
        selected_list_index: &mut Option<usize>,
        cursor: Option<LogCursor>,
        commit_idx: usize,
        file_idx: usize,
    ) {
        if diff.binary {
            items.push(ListItem::new(Line::from(Span::styled(
                "      Binary file (not shown)",
                Style::default()
                    .fg(self.config.theme.diff_no_newline)
                    .add_modifier(Modifier::ITALIC),
            ))));
            return;
        }

        let renderer = DiffRenderer::new(self.config);
        let render_context = crate::ui::diff_renderer::DiffRenderContext::new(&renderer, diff);

        for (hunk_idx, hunk) in diff.hunks.iter().enumerate() {
            let is_collapsed = self
                .navigation
                .is_hunk_collapsed(commit_idx, file_idx, hunk_idx);
            let is_hunk_selected = matches!(
                cursor,
                Some(LogCursor::Hunk { commit_index, file_index, hunk_index })
                if commit_index == commit_idx && file_index == file_idx && hunk_index == hunk_idx
            );

            if is_hunk_selected {
                *selected_list_index = Some(items.len());
            }

            let header_style = if is_hunk_selected {
                Style::default()
                    .bg(self.config.theme.selected_bg)
                    .fg(self.config.theme.selected_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                self.config.theme.diff_hunk_header
            };

            let collapse_indicator = if is_collapsed { "▸" } else { "▾" };
            items.push(ListItem::new(Line::from(Span::styled(
                format!("      {} {}", collapse_indicator, hunk.header.raw),
                header_style,
            ))));

            if !is_collapsed {
                let mut highlighter = render_context.create_fresh_highlighter();

                for line in &hunk.lines {
                    let formatted_line = render_context.format_diff_line(
                        line,
                        is_hunk_selected,
                        Some("        "),
                        highlighter.as_mut(),
                    );
                    items.push(ListItem::new(formatted_line));
                }
            }

            if diff.hunks.len() > 1 {
                items.push(ListItem::new(Line::from("")));
            }
        }
    }
}

fn format_relative_time(time: &git2::Time) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let commit_time = time.seconds();
    let delta = now - commit_time;

    if delta < 0 {
        return "in the future".to_string();
    }
    if delta < 60 {
        return "just now".to_string();
    }
    if delta < 3600 {
        let mins = delta / 60;
        return if mins == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{} minutes ago", mins)
        };
    }
    if delta < 86400 {
        let hours = delta / 3600;
        return if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", hours)
        };
    }
    if delta < 604800 {
        let days = delta / 86400;
        return if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{} days ago", days)
        };
    }
    if delta < 2592000 {
        let weeks = delta / 604800;
        return if weeks == 1 {
            "1 week ago".to_string()
        } else {
            format!("{} weeks ago", weeks)
        };
    }
    if delta < 31536000 {
        let months = delta / 2592000;
        return if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{} months ago", months)
        };
    }
    let years = delta / 31536000;
    if years == 1 {
        "1 year ago".to_string()
    } else {
        format!("{} years ago", years)
    }
}
