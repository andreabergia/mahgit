use crate::config::Config;
use crate::diff::{Diff, LineType};
use crate::status::{FileEntry, RepositoryStatus};
use crate::ui::navigation::{FileDiffKey, NavigationState, SelectionCursor, StatusSection};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

const ICON_COLLAPSED: &str = "▸";
const ICON_EXPANDED: &str = "▾";

struct ListRenderContext<'a> {
    selected_list_index: &'a mut Option<usize>,
    scroll_targets: &'a mut Vec<usize>,
    track_visibility: bool,
}

impl<'a> ListRenderContext<'a> {
    fn new(
        selected_list_index: &'a mut Option<usize>,
        scroll_targets: &'a mut Vec<usize>,
        track_visibility: bool,
    ) -> Self {
        Self {
            selected_list_index,
            scroll_targets,
            track_visibility,
        }
    }

    fn select_index(&mut self, index: usize) {
        *self.selected_list_index = Some(index);
    }

    fn ensure_visible(&mut self, index: usize) {
        if self.track_visibility {
            self.scroll_targets.push(index);
        }
    }
}

pub struct StatusView<'a> {
    status: &'a RepositoryStatus,
    navigation: &'a NavigationState,
    config: &'a Config,
}

impl<'a> StatusView<'a> {
    pub fn new(
        status: &'a RepositoryStatus,
        navigation: &'a NavigationState,
        config: &'a Config,
    ) -> Self {
        Self {
            status,
            navigation,
            config,
        }
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
        let mut selected_list_index = None;
        let mut scroll_targets: Vec<usize> = Vec::new();
        let track_visibility = !self.navigation.is_manual_scroll_active();
        let cursor = self.navigation.current_cursor();
        let mut render_ctx = ListRenderContext::new(
            &mut selected_list_index,
            &mut scroll_targets,
            track_visibility,
        );

        self.add_section_items(
            &mut items,
            &mut render_ctx,
            StatusSection::Conflicted,
            "Conflicted files",
            self.status.conflicted_files(),
            cursor,
        );

        self.add_section_items_with_entries(
            &mut items,
            &mut render_ctx,
            StatusSection::Unstaged,
            "Unstaged changes",
            self.status.unstaged_files(),
            cursor,
        );

        self.add_section_items(
            &mut items,
            &mut render_ctx,
            StatusSection::Untracked,
            "Untracked files",
            self.status.untracked_files(),
            cursor,
        );

        self.add_section_items_with_entries(
            &mut items,
            &mut render_ctx,
            StatusSection::Staged,
            "Staged changes",
            self.status.staged_files(),
            cursor,
        );

        let viewport_height = area.height as usize;
        self.navigation
            .update_viewport_metrics(viewport_height, items.len(), &scroll_targets);

        let list = List::new(items);

        let mut list_state = ratatui::widgets::ListState::default();
        // When manual scrolling is active, don't set selection to prevent
        // the List widget from auto-scrolling to keep the selected item visible
        if !self.navigation.is_manual_scroll_active() {
            list_state.select(selected_list_index);
        }
        *list_state.offset_mut() = self.navigation.scroll_offset();

        f.render_stateful_widget(list, area, &mut list_state);
    }

    fn add_section_items_with_entries(
        &self,
        items: &mut Vec<ListItem>,
        render_ctx: &mut ListRenderContext,
        section: StatusSection,
        header: &str,
        entries: &[FileEntry],
        cursor: Option<SelectionCursor>,
    ) {
        if entries.is_empty() {
            return;
        }

        let is_collapsed = self.navigation.is_section_collapsed(section);
        let collapse_icon = if is_collapsed {
            ICON_COLLAPSED
        } else {
            ICON_EXPANDED
        };
        let section_header = format!("{} {} ({})", collapse_icon, header, entries.len());
        let header_style = self.config.theme.section_header;

        items.push(ListItem::new(Line::from(Span::styled(
            section_header,
            header_style,
        ))));

        // Only show files if section is not collapsed
        if !is_collapsed {
            for (file_index, entry) in entries.iter().enumerate() {
                let is_selected = matches!(cursor, Some(SelectionCursor::File { section: s, file_index: idx }) if s == section && idx == file_index);

                // Track the list index of the selected item
                if is_selected {
                    let index = items.len();
                    render_ctx.select_index(index);
                    render_ctx.ensure_visible(index);
                }

                let style = if is_selected {
                    Style::default()
                        .bg(self.config.theme.selected_bg)
                        .fg(self.config.theme.selected_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.get_file_style(section)
                };

                let diff_key = FileDiffKey::new(entry.path.clone(), section.into());
                let (icon, icon_style) = self.inline_diff_indicator(&diff_key, is_selected);

                let mut spans = Vec::new();
                spans.push(if is_selected {
                    Span::styled(
                        "> ",
                        Style::default().fg(self
                            .config
                            .theme
                            .diff_hunk_header_focused
                            .fg
                            .unwrap_or(Color::Yellow)),
                    )
                } else {
                    Span::raw("  ")
                });
                spans.push(Span::raw("  "));
                spans.push(Span::styled(format!("{} ", icon), icon_style));

                // Format renamed files as "renamed: old_path -> new_path"
                let file_display = if let Some(old_path) = &entry.old_path {
                    format!("{} {} -> {}", entry.status, old_path, entry.path)
                } else {
                    format!("{} {}", entry.status, entry.path)
                };

                spans.push(Span::styled(file_display, style));

                items.push(ListItem::new(Line::from(spans)));

                // Add inline diff content if file diff is expanded
                if let Some(diff_state) = self.navigation.get_file_diff(&diff_key)
                    && diff_state.expanded
                {
                    if let Some(diff) = &diff_state.diff {
                        self.add_inline_diff_items_with_selection(
                            items, diff, render_ctx, section, file_index, cursor,
                        );
                    } else {
                        // Show loading placeholder
                        items.push(ListItem::new(Line::from(Span::styled(
                            "    Loading diff...",
                            Style::default()
                                .fg(self.config.theme.diff_no_newline)
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
        render_ctx: &mut ListRenderContext,
        section: StatusSection,
        header: &str,
        files: &[String],
        cursor: Option<SelectionCursor>,
    ) {
        if files.is_empty() {
            return;
        }

        let is_collapsed = self.navigation.is_section_collapsed(section);
        let collapse_icon = if is_collapsed {
            ICON_COLLAPSED
        } else {
            ICON_EXPANDED
        };
        let section_header = format!("{} {} ({})", collapse_icon, header, files.len());
        let header_style = self.config.theme.section_header;

        items.push(ListItem::new(Line::from(Span::styled(
            section_header,
            header_style,
        ))));

        // Only show files if section is not collapsed
        if !is_collapsed {
            for (file_index, file) in files.iter().enumerate() {
                let display_name = match section {
                    StatusSection::Untracked => file.as_str(),
                    _ => std::path::Path::new(file)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(file),
                };
                let is_selected = matches!(cursor, Some(SelectionCursor::File { section: s, file_index: idx }) if s == section && idx == file_index);

                // Track the list index of the selected item
                if is_selected {
                    let index = items.len();
                    render_ctx.select_index(index);
                    render_ctx.ensure_visible(index);
                }

                let style = if is_selected {
                    Style::default()
                        .bg(self.config.theme.selected_bg)
                        .fg(self.config.theme.selected_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.get_file_style(section)
                };

                let diff_key = FileDiffKey::new(file.clone(), section.into());
                let (icon, icon_style) = self.inline_diff_indicator(&diff_key, is_selected);
                let file_indicator = self.get_file_indicator(section);

                let mut spans = Vec::new();
                spans.push(if is_selected {
                    Span::styled(
                        "> ",
                        Style::default().fg(self
                            .config
                            .theme
                            .diff_hunk_header_focused
                            .fg
                            .unwrap_or(Color::Yellow)),
                    )
                } else {
                    Span::raw("  ")
                });
                spans.push(Span::raw("  "));
                spans.push(Span::styled(format!("{} ", icon), icon_style));
                spans.push(Span::styled(
                    format!("{} {}", file_indicator, display_name),
                    style,
                ));

                items.push(ListItem::new(Line::from(spans)));

                // Add inline diff content if file diff is expanded
                if let Some(diff_state) = self.navigation.get_file_diff(&diff_key)
                    && diff_state.expanded
                {
                    if let Some(diff) = &diff_state.diff {
                        self.add_inline_diff_items_with_selection(
                            items, diff, render_ctx, section, file_index, cursor,
                        );
                    } else {
                        // Show loading placeholder
                        items.push(ListItem::new(Line::from(Span::styled(
                            "    Loading diff...",
                            Style::default()
                                .fg(self.config.theme.diff_no_newline)
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
            StatusSection::Staged => Style::default().fg(self.config.theme.staged),
            StatusSection::Unstaged => Style::default().fg(self.config.theme.unstaged),
            StatusSection::Untracked => Style::default().fg(self.config.theme.untracked),
            StatusSection::Conflicted => Style::default().fg(self.config.theme.conflicted),
        }
    }

    fn add_inline_diff_items_with_selection(
        &self,
        items: &mut Vec<ListItem>,
        diff: &Diff,
        render_ctx: &mut ListRenderContext,
        section: StatusSection,
        file_index: usize,
        cursor: Option<SelectionCursor>,
    ) {
        // Check for binary files
        if diff.binary {
            items.push(ListItem::new(Line::from(Span::styled(
                "    Binary file (not shown)",
                Style::default()
                    .fg(self.config.theme.diff_no_newline)
                    .add_modifier(Modifier::ITALIC),
            ))));
            return;
        }

        for (idx, hunk) in diff.hunks.iter().enumerate() {
            // Check if this hunk is collapsed
            let key = FileDiffKey::new(diff.file_path.clone(), section.into());
            let is_collapsed = self.navigation.is_hunk_collapsed(&key, idx);

            // Hunk header with indentation and selection highlight
            let header_index = items.len();
            let is_selected = matches!(cursor, Some(SelectionCursor::Hunk { section: s, file_index: fi, hunk_index: hi }) if s == section && fi == file_index && hi == idx);
            let mut header_style = if is_selected {
                self.config.theme.diff_hunk_header_focused
            } else {
                self.config.theme.diff_hunk_header
            };

            // Apply subtle background color for selected hunk if available
            if is_selected {
                if let Some(bg_color) = self.config.theme.selected_hunk_bg {
                    header_style = header_style.bg(bg_color);
                }
                render_ctx.ensure_visible(header_index);
                render_ctx.select_index(header_index);
            }

            // Add collapse indicator
            let collapse_indicator = if is_collapsed {
                ICON_COLLAPSED
            } else {
                ICON_EXPANDED
            };
            items.push(ListItem::new(Line::from(Span::styled(
                format!("    {} {}", collapse_indicator, hunk.header.raw),
                header_style,
            ))));

            // Only show diff lines if not collapsed
            if !is_collapsed {
                // Diff lines with deeper indentation
                for line in &hunk.lines {
                    let (prefix, color) = match line.line_type {
                        LineType::Addition => ("+", self.config.theme.staged),
                        LineType::Deletion => ("-", self.config.theme.unstaged),
                        LineType::Context => (" ", self.config.theme.diff_context),
                        LineType::NoNewlineEOF => ("\\", self.config.theme.diff_no_newline),
                    };

                    // Expand tabs in the line content
                    let expanded_content = self.expand_tabs(&line.content);

                    // Apply subtle background to diff lines in selected hunk
                    let mut line_style = Style::default().fg(color);
                    if is_selected
                        && let Some(bg_color) = self.config.theme.selected_hunk_bg
                    {
                        line_style = line_style.bg(bg_color);
                    }

                    items.push(ListItem::new(Line::from(Span::styled(
                        format!("      {}{}", prefix, expanded_content),
                        line_style,
                    ))));
                }
            }

            if diff.hunks.len() > 1 {
                items.push(ListItem::new(Line::from("")));
            }
        }
    }
}

impl<'a> StatusView<'a> {
    fn inline_diff_indicator(&self, key: &FileDiffKey, is_selected: bool) -> (&'static str, Style) {
        let expanded = self.navigation.is_file_diff_expanded(key);
        let mut icon_style = if expanded {
            Style::default().fg(self
                .config
                .theme
                .diff_hunk_header_focused
                .fg
                .unwrap_or(Color::Yellow))
        } else {
            Style::default().fg(self.config.theme.diff_no_newline)
        };

        if is_selected {
            icon_style = icon_style
                .bg(self.config.theme.selected_bg)
                .add_modifier(Modifier::BOLD);
        }

        let icon = if expanded {
            ICON_EXPANDED
        } else {
            ICON_COLLAPSED
        };

        (icon, icon_style)
    }

    fn expand_tabs(&self, line: &str) -> String {
        crate::config::expand_tabs_with_width(line, self.config.tab_width)
    }
}
