use crate::diff::{Diff, LineType};
use crate::status::{FileEntry, RepositoryStatus};
use crate::ui::navigation::{FileDiffKey, NavigationFocus, NavigationState, StatusSection};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub struct StatusView<'a> {
    status: &'a RepositoryStatus,
    navigation: &'a NavigationState,
}

impl<'a> StatusView<'a> {
    pub fn new(status: &'a RepositoryStatus, navigation: &'a NavigationState) -> Self {
        Self { status, navigation }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .constraints([Constraint::Min(3)].as_ref())
            .split(area);

        let repo_name = std::env::current_dir()
            .ok()
            .and_then(|p| {
                p.file_name()
                    .and_then(|n| n.to_str().map(|s| s.to_string()))
            })
            .unwrap_or_else(|| "unknown".to_string());

        let title = format!(
            "Repository: {} (branch: {})",
            repo_name, self.status.branch_name
        );

        let block = Block::default().title(title).borders(Borders::ALL);

        let inner_area = block.inner(chunks[0]);
        f.render_widget(block, chunks[0]);

        // Add margin around the content (1 row/column on all sides)
        let content_area = inner_area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });

        if self.status.is_clean() {
            let clean_text = Text::from("Working directory is clean");
            let paragraph = Paragraph::new(clean_text);
            f.render_widget(paragraph, content_area);
        } else {
            self.render_file_sections(f, content_area);
        }
    }

    fn render_file_sections(&self, f: &mut Frame, area: Rect) {
        let mut items = Vec::new();
        let mut current_file_index = 0;
        let mut selected_list_index = None;

        self.add_section_items(
            &mut items,
            &mut current_file_index,
            &mut selected_list_index,
            StatusSection::Conflicted,
            "Conflicted files",
            self.status.conflicted_files(),
        );

        self.add_section_items_with_entries(
            &mut items,
            &mut current_file_index,
            &mut selected_list_index,
            StatusSection::Unstaged,
            "Unstaged changes",
            self.status.unstaged_files(),
        );

        self.add_section_items(
            &mut items,
            &mut current_file_index,
            &mut selected_list_index,
            StatusSection::Untracked,
            "Untracked files",
            self.status.untracked_files(),
        );

        self.add_section_items_with_entries(
            &mut items,
            &mut current_file_index,
            &mut selected_list_index,
            StatusSection::Staged,
            "Staged changes",
            self.status.staged_files(),
        );

        let list = List::new(items);

        // Create a list state that will handle scrolling automatically
        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(selected_list_index);

        f.render_stateful_widget(list, area, &mut list_state);
    }

    fn add_section_items_with_entries(
        &self,
        items: &mut Vec<ListItem>,
        current_file_index: &mut usize,
        selected_list_index: &mut Option<usize>,
        section: StatusSection,
        header: &str,
        entries: &[FileEntry],
    ) {
        if entries.is_empty() {
            return;
        }

        let is_collapsed = self.navigation.is_section_collapsed(section);
        let collapse_icon = if is_collapsed { "▶" } else { "▼" };
        let section_header = format!("{} {} ({})", collapse_icon, header, entries.len());
        let header_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);

        items.push(ListItem::new(Line::from(Span::styled(
            section_header,
            header_style,
        ))));

        // Only show files if section is not collapsed
        if !is_collapsed {
            for (file_index, entry) in entries.iter().enumerate() {
                let content = format!("  {} {}", entry.status, entry.path);

                let is_selected = self.navigation.current_section() == section
                    && self.navigation.selected_index() == file_index;

                // Track the list index of the selected item
                if is_selected {
                    *selected_list_index = Some(items.len());
                }

                let style = if is_selected {
                    Style::default()
                        .bg(Color::DarkGray)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.get_file_style(section)
                };

                let mut spans = vec![Span::styled(content, style)];

                if is_selected {
                    spans.insert(0, Span::styled("> ", Style::default().fg(Color::Yellow)));
                } else {
                    spans.insert(0, Span::raw("  "));
                }

                items.push(ListItem::new(Line::from(spans)));
                *current_file_index += 1;

                // Add inline diff content if file diff is expanded
                let diff_key = FileDiffKey::new(entry.path.clone(), section.into());
                if let Some(diff_state) = self.navigation.get_file_diff(&diff_key)
                    && diff_state.expanded
                {
                    if let Some(diff) = &diff_state.diff {
                        self.add_inline_diff_items_with_selection(
                            items,
                            diff,
                            diff_state.current_hunk,
                        );
                    } else {
                        // Show loading placeholder
                        items.push(ListItem::new(Line::from(Span::styled(
                            "    Loading diff...",
                            Style::default()
                                .fg(Color::Gray)
                                .add_modifier(Modifier::ITALIC),
                        ))));
                    }
                }
            }
        }

        items.push(ListItem::new(Line::from("")));
    }

    fn add_section_items(
        &self,
        items: &mut Vec<ListItem>,
        current_file_index: &mut usize,
        selected_list_index: &mut Option<usize>,
        section: StatusSection,
        header: &str,
        files: &[String],
    ) {
        if files.is_empty() {
            return;
        }

        let is_collapsed = self.navigation.is_section_collapsed(section);
        let collapse_icon = if is_collapsed { "▶" } else { "▼" };
        let section_header = format!("{} {} ({})", collapse_icon, header, files.len());
        let header_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);

        items.push(ListItem::new(Line::from(Span::styled(
            section_header,
            header_style,
        ))));

        // Only show files if section is not collapsed
        if !is_collapsed {
            for (file_index, file) in files.iter().enumerate() {
                let file_name = std::path::Path::new(file)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(file);
                let file_indicator = self.get_file_indicator(section);
                let content = format!("  {} {}", file_indicator, file_name);

                let is_selected = self.navigation.current_section() == section
                    && self.navigation.selected_index() == file_index;

                // Track the list index of the selected item
                if is_selected {
                    *selected_list_index = Some(items.len());
                }

                let style = if is_selected {
                    Style::default()
                        .bg(Color::DarkGray)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.get_file_style(section)
                };

                let mut spans = vec![Span::styled(content, style)];

                if is_selected {
                    spans.insert(0, Span::styled("> ", Style::default().fg(Color::Yellow)));
                } else {
                    spans.insert(0, Span::raw("  "));
                }

                items.push(ListItem::new(Line::from(spans)));
                *current_file_index += 1;

                // Add inline diff content if file diff is expanded
                let diff_key = FileDiffKey::new(file.clone(), section.into());
                if let Some(diff_state) = self.navigation.get_file_diff(&diff_key)
                    && diff_state.expanded
                {
                    if let Some(diff) = &diff_state.diff {
                        self.add_inline_diff_items_with_selection(
                            items,
                            diff,
                            diff_state.current_hunk,
                        );
                    } else {
                        // Show loading placeholder
                        items.push(ListItem::new(Line::from(Span::styled(
                            "    Loading diff...",
                            Style::default()
                                .fg(Color::Gray)
                                .add_modifier(Modifier::ITALIC),
                        ))));
                    }
                }
            }
        }

        items.push(ListItem::new(Line::from("")));
    }

    fn get_file_indicator(&self, section: StatusSection) -> &'static str {
        match section {
            StatusSection::Staged => "staged", // This shouldn't be used anymore for staged files
            StatusSection::Unstaged => "modified", // This shouldn't be used anymore for unstaged files
            StatusSection::Untracked => "new",
            StatusSection::Conflicted => "conflict",
        }
    }

    fn get_file_style(&self, section: StatusSection) -> Style {
        match section {
            StatusSection::Staged => Style::default().fg(Color::Green),
            StatusSection::Unstaged => Style::default().fg(Color::Red),
            StatusSection::Untracked => Style::default().fg(Color::Magenta),
            StatusSection::Conflicted => Style::default().fg(Color::Yellow),
        }
    }

    fn add_inline_diff_items_with_selection(
        &self,
        items: &mut Vec<ListItem>,
        diff: &Diff,
        selected_hunk: usize,
    ) {
        let diff_focused = self.navigation.focus() == NavigationFocus::InlineDiff;
        // Check for binary files
        if diff.binary {
            items.push(ListItem::new(Line::from(Span::styled(
                "    Binary file (not shown)",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            ))));
            return;
        }

        for (idx, hunk) in diff.hunks.iter().enumerate() {
            // Hunk header with indentation and selection highlight
            let header_style = if diff_focused && idx == selected_hunk {
                Style::default().fg(Color::Cyan).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM)
            };
            items.push(ListItem::new(Line::from(Span::styled(
                format!("    {}", hunk.header.raw),
                header_style,
            ))));

            // Diff lines with deeper indentation
            for line in &hunk.lines {
                let (prefix, color) = match line.line_type {
                    LineType::Addition => ("+", Color::Green),
                    LineType::Deletion => ("-", Color::Red),
                    LineType::Context => (" ", Color::White),
                    LineType::NoNewlineEOF => ("\\", Color::Yellow),
                };

                items.push(ListItem::new(Line::from(Span::styled(
                    format!("      {}{}", prefix, line.content),
                    Style::default().fg(color),
                ))));
            }

            if diff.hunks.len() > 1 {
                items.push(ListItem::new(Line::from("")));
            }
        }
    }
}
