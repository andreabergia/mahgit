use crate::config::Config;
use crate::log::{LogData, LogEntry};
use crate::repository::CommitChangeType;
use crate::theme::Theme;
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

            // If commit is expanded, show metadata block then changed files
            if expanded && let Some(expansion) = self.navigation.get_expansion(commit_idx) {
                for line in commit_metadata_lines(entry, &self.config.theme) {
                    items.push(ListItem::new(line));
                }

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

/// Indentation for the metadata block — sits between the commit row (2-space
/// prefix) and the file rows (4-space prefix).
const METADATA_INDENT: &str = "      ";
/// Width the label column is padded to so values line up ("Committer" is widest).
const METADATA_LABEL_WIDTH: usize = 9;

/// Full commit message split into display lines (trailing blank lines trimmed).
fn commit_message_lines(message: &str) -> Vec<&str> {
    message.trim_end().split('\n').collect()
}

/// Number of list lines the metadata block occupies for a commit. Single source
/// of truth shared with `commit_metadata_lines` and the flat-index accounting in
/// `log_navigation`.
pub fn commit_metadata_line_count(entry: &LogEntry) -> usize {
    // Commit + Author, optional Committer, a blank separator, then the message.
    2 + usize::from(entry.has_distinct_committer()) + 1 + commit_message_lines(&entry.message).len()
}

/// Build the metadata block shown under an expanded commit row.
pub fn commit_metadata_lines(entry: &LogEntry, theme: &Theme) -> Vec<Line<'static>> {
    let label_style = Style::default().fg(theme.log_date);

    let labeled = |label: &str, value_spans: Vec<Span<'static>>| -> Line<'static> {
        let mut spans = vec![Span::styled(
            // Trailing space guarantees a gap even for the widest label.
            format!(
                "{}{:<width$} ",
                METADATA_INDENT,
                label,
                width = METADATA_LABEL_WIDTH
            ),
            label_style,
        )];
        spans.extend(value_spans);
        Line::from(spans)
    };

    let identity_spans = |name: &str, email: &str, time: &git2::Time| -> Vec<Span<'static>> {
        vec![
            Span::styled(
                format!("{} <{}>", name, email),
                Style::default().fg(theme.log_author),
            ),
            Span::styled(
                format!("  {}", format_absolute_time(time)),
                Style::default().fg(theme.log_date),
            ),
        ]
    };

    let mut lines = Vec::with_capacity(commit_metadata_line_count(entry));

    lines.push(labeled(
        "Commit",
        vec![Span::styled(
            entry.oid.to_string(),
            Style::default().fg(theme.log_hash),
        )],
    ));
    lines.push(labeled(
        "Author",
        identity_spans(&entry.author_name, &entry.author_email, &entry.time),
    ));
    if entry.has_distinct_committer() {
        lines.push(labeled(
            "Committer",
            identity_spans(
                &entry.committer_name,
                &entry.committer_email,
                &entry.committer_time,
            ),
        ));
    }

    lines.push(Line::from(""));

    for msg_line in commit_message_lines(&entry.message) {
        lines.push(Line::from(Span::styled(
            format!("{}{}", METADATA_INDENT, msg_line),
            Style::default().fg(theme.diff_context),
        )));
    }

    lines
}

/// Format a git2 timestamp as `YYYY-MM-DD HH:MM ±HHMM`, applying the commit's
/// own recorded timezone offset (dependency-free, no `chrono`/`time` crate).
fn format_absolute_time(time: &git2::Time) -> String {
    let offset_minutes = time.offset_minutes() as i64;
    let local = time.seconds() + offset_minutes * 60;

    let days = local.div_euclid(86400);
    let secs_of_day = local.rem_euclid(86400);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;

    let (year, month, day) = civil_from_days(days);

    let sign = if offset_minutes < 0 { '-' } else { '+' };
    let off_abs = offset_minutes.abs();

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02} {}{:02}{:02}",
        year,
        month,
        day,
        hour,
        minute,
        sign,
        off_abs / 60,
        off_abs % 60,
    )
}

/// Convert a count of days since the Unix epoch into a `(year, month, day)`
/// triple (Howard Hinnant's civil-from-days algorithm).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn format_relative_time(time: &git2::Time) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    relative_time(now, time.seconds())
}

fn relative_time(now: i64, commit_time: i64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::LogEntry;

    fn sample_entry() -> LogEntry {
        LogEntry {
            oid: git2::Oid::zero(),
            short_hash: "0000000".to_string(),
            summary: "summary line".to_string(),
            message: "summary line\n\nbody line one\nbody line two\n".to_string(),
            author_name: "Alice".to_string(),
            author_email: "alice@example.com".to_string(),
            time: git2::Time::new(1000, 0),
            committer_name: "Alice".to_string(),
            committer_email: "alice@example.com".to_string(),
            committer_time: git2::Time::new(1000, 0),
        }
    }

    #[test]
    fn metadata_line_count_matches_rendered_lines_without_committer() {
        let entry = sample_entry();
        let theme = Theme::default();
        assert!(!entry.has_distinct_committer());
        assert_eq!(
            commit_metadata_lines(&entry, &theme).len(),
            commit_metadata_line_count(&entry)
        );
    }

    #[test]
    fn metadata_line_count_matches_rendered_lines_with_committer() {
        let mut entry = sample_entry();
        entry.committer_name = "Bob".to_string();
        entry.committer_time = git2::Time::new(2000, 0);
        let theme = Theme::default();
        assert!(entry.has_distinct_committer());
        assert_eq!(
            commit_metadata_lines(&entry, &theme).len(),
            commit_metadata_line_count(&entry)
        );
    }

    #[test]
    fn metadata_omits_committer_line_when_identical() {
        let entry = sample_entry();
        let theme = Theme::default();
        let text: Vec<String> = commit_metadata_lines(&entry, &theme)
            .iter()
            .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert!(text.iter().any(|l: &String| l.contains("Author")));
        assert!(!text.iter().any(|l: &String| l.contains("Committer")));
    }

    #[test]
    fn metadata_includes_committer_line_when_distinct() {
        let mut entry = sample_entry();
        entry.committer_name = "Bob".to_string();
        let theme = Theme::default();
        let text: Vec<String> = commit_metadata_lines(&entry, &theme)
            .iter()
            .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert!(text.iter().any(|l: &String| l.contains("Committer")));
        assert!(text.iter().any(|l: &String| l.contains("Bob")));
    }

    #[test]
    fn format_absolute_time_epoch_utc() {
        assert_eq!(
            format_absolute_time(&git2::Time::new(0, 0)),
            "1970-01-01 00:00 +0000"
        );
    }

    #[test]
    fn format_absolute_time_known_value() {
        // 2021-01-01 00:00:00 UTC = 1609459200
        assert_eq!(
            format_absolute_time(&git2::Time::new(1609459200, 0)),
            "2021-01-01 00:00 +0000"
        );
    }

    #[test]
    fn format_absolute_time_applies_offset() {
        // Same instant, +120 minute offset shifts displayed local time to 02:00.
        assert_eq!(
            format_absolute_time(&git2::Time::new(1609459200, 120)),
            "2021-01-01 02:00 +0200"
        );
        // Negative offset.
        assert_eq!(
            format_absolute_time(&git2::Time::new(1609459200, -300)),
            "2020-12-31 19:00 -0500"
        );
    }

    #[test]
    fn test_relative_time_future() {
        assert_eq!(relative_time(100, 200), "in the future");
    }

    #[test]
    fn test_relative_time_just_now() {
        assert_eq!(relative_time(1000, 1000), "just now");
        assert_eq!(relative_time(1059, 1000), "just now");
    }

    #[test]
    fn test_relative_time_minutes() {
        assert_eq!(relative_time(1060, 1000), "1 minute ago");
        assert_eq!(relative_time(1120, 1000), "2 minutes ago");
    }

    #[test]
    fn test_relative_time_hours() {
        assert_eq!(relative_time(3600, 0), "1 hour ago");
        assert_eq!(relative_time(7200, 0), "2 hours ago");
    }

    #[test]
    fn test_relative_time_days_weeks_months_years() {
        assert_eq!(relative_time(86400, 0), "1 day ago");
        assert_eq!(relative_time(604800, 0), "1 week ago");
        assert_eq!(relative_time(2592000, 0), "1 month ago");
        assert_eq!(relative_time(31536000, 0), "1 year ago");
        assert_eq!(relative_time(63072000, 0), "2 years ago");
    }

    #[test]
    fn test_relative_time_clock_before_epoch_does_not_panic() {
        // now defaults to 0 when the system clock is before the Unix epoch;
        // a commit at a positive time then reads as "in the future" rather
        // than panicking the render loop.
        assert_eq!(relative_time(0, 1000), "in the future");
    }
}
