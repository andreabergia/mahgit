use crate::config::Config;
use crate::diff::Diff;
use crate::status::{FileEntry, RepositoryStatus};
use crate::ui::diff_renderer::{DiffRenderer, SearchHighlight};
use crate::ui::diff_search::DiffSearchMode;
use crate::ui::navigation::{
    FileDiffKey, InlineDiffState, NavigationState, SelectionCursor, StatusSection,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::collections::HashMap;

const ICON_COLLAPSED: &str = "▸";
const ICON_EXPANDED: &str = "▾";

struct ListRenderContext<'a> {
    selected_list_index: &'a mut Option<usize>,
    scroll_targets: &'a mut Vec<usize>,
    track_visibility: bool,
}

#[derive(Debug, Clone)]
struct HunkOverlay {
    start_line: usize,
    end_line: usize,
    section_label: String,
    file_path: String,
    header: String,
    selected: bool,
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
        let list_area = area;

        let mut items = Vec::new();
        let mut selected_list_index = None;
        let mut scroll_targets: Vec<usize> = Vec::new();
        let track_visibility = !self.navigation.is_manual_scroll_active();
        let cursor = self.navigation.current_cursor();
        let mut overlays: Vec<HunkOverlay> = Vec::new();
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
            &mut overlays,
        );

        self.add_section_items_with_entries(
            &mut items,
            &mut render_ctx,
            StatusSection::Unstaged,
            "Unstaged changes",
            self.status.unstaged_files(),
            cursor,
            &mut overlays,
        );

        self.add_section_items(
            &mut items,
            &mut render_ctx,
            StatusSection::Untracked,
            "Untracked files",
            self.status.untracked_files(),
            cursor,
            &mut overlays,
        );

        self.add_section_items_with_entries(
            &mut items,
            &mut render_ctx,
            StatusSection::Staged,
            "Staged changes",
            self.status.staged_files(),
            cursor,
            &mut overlays,
        );

        let visible_start = self.navigation.scroll_offset();
        let active_overlay = overlays
            .iter()
            .find(|o| {
                o.selected
                    && ranges_intersect(
                        o.start_line,
                        o.end_line,
                        visible_start,
                        visible_start.saturating_add(list_area.height as usize),
                    )
            })
            .or_else(|| {
                overlays.iter().find(|o| {
                    ranges_intersect(
                        o.start_line,
                        o.end_line,
                        visible_start,
                        visible_start.saturating_add(list_area.height as usize),
                    )
                })
            })
            .or_else(|| overlays.first());

        let sticky_header_height = if let Some(active) = active_overlay {
            if active.start_line < visible_start && list_area.height > 1 {
                1
            } else {
                0
            }
        } else {
            0
        };
        let list_draw_area = Rect {
            x: list_area.x,
            y: list_area.y + sticky_header_height,
            width: list_area.width,
            height: list_area.height.saturating_sub(sticky_header_height),
        };

        let viewport_height = list_draw_area.height as usize;
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

        if list_draw_area.height > 0 {
            f.render_stateful_widget(list, list_draw_area, &mut list_state);
        }

        if sticky_header_height > 0
            && let Some(active) = active_overlay
        {
            let header_area = Rect {
                x: list_area.x,
                y: list_area.y,
                width: list_area.width,
                height: sticky_header_height,
            };
            self.render_sticky_overlay(f, header_area, active);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_section_items_with_entries(
        &self,
        items: &mut Vec<ListItem>,
        render_ctx: &mut ListRenderContext,
        section: StatusSection,
        header: &str,
        entries: &[FileEntry],
        cursor: Option<SelectionCursor>,
        overlays: &mut Vec<HunkOverlay>,
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
                            .diff_hunk_header
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
                            items, diff, diff_state, render_ctx, section, file_index, cursor,
                            overlays,
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

    #[allow(clippy::too_many_arguments)]
    fn add_section_items(
        &self,
        items: &mut Vec<ListItem>,
        render_ctx: &mut ListRenderContext,
        section: StatusSection,
        header: &str,
        files: &[String],
        cursor: Option<SelectionCursor>,
        overlays: &mut Vec<HunkOverlay>,
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
                            .diff_hunk_header
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
                            items, diff, diff_state, render_ctx, section, file_index, cursor,
                            overlays,
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

    #[allow(clippy::too_many_arguments)]
    fn add_inline_diff_items_with_selection(
        &self,
        items: &mut Vec<ListItem>,
        diff: &Diff,
        diff_state: &InlineDiffState,
        render_ctx: &mut ListRenderContext,
        section: StatusSection,
        file_index: usize,
        cursor: Option<SelectionCursor>,
        overlays: &mut Vec<HunkOverlay>,
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

        let search_state = &diff_state.search_state;
        let search_active = !matches!(search_state.mode, DiffSearchMode::Inactive);
        let is_search_context = matches!(
            cursor,
            Some(SelectionCursor::File { section: s, file_index: fi, .. })
                | Some(SelectionCursor::Hunk {
                    section: s,
                    file_index: fi,
                    ..
                }) if s == section && fi == file_index
        );
        let mut matches_by_line: HashMap<(usize, usize), Vec<SearchHighlight>> = HashMap::new();
        let mut matches_by_hunk: HashMap<usize, usize> = HashMap::new();

        if search_active {
            for (idx, m) in search_state.matches.iter().enumerate() {
                matches_by_line
                    .entry((m.hunk_index, m.line_index))
                    .or_default()
                    .push(SearchHighlight {
                        start: m.column,
                        length: m.length,
                        is_active: search_state.active_match_index == Some(idx),
                    });
                *matches_by_hunk.entry(m.hunk_index).or_insert(0) += 1;
            }

            for highlights in matches_by_line.values_mut() {
                highlights.sort_by_key(|h| h.start);
            }
        }

        let renderer = DiffRenderer::new(self.config);
        // Create render context once for this diff - handles syntax highlighting automatically
        let render_context = crate::ui::diff_renderer::DiffRenderContext::new(&renderer, diff);

        for (idx, hunk) in diff.hunks.iter().enumerate() {
            // Check if this hunk is collapsed
            let key = FileDiffKey::new(diff.file_path.clone(), section.into());
            let is_collapsed = self.navigation.is_hunk_collapsed(&key, idx);

            // Hunk header with indentation and selection highlight
            let start_line = items.len();
            let is_selected = matches!(cursor, Some(SelectionCursor::Hunk { section: s, file_index: fi, hunk_index: hi }) if s == section && fi == file_index && hi == idx);
            let header_style = if is_selected {
                Style::default()
                    .bg(self.config.theme.selected_bg)
                    .fg(self.config.theme.selected_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                self.config.theme.diff_hunk_header
            };

            if is_selected {
                render_ctx.ensure_visible(start_line);
                render_ctx.select_index(start_line);
            }

            // Add collapse indicator
            let collapse_indicator = if is_collapsed {
                ICON_COLLAPSED
            } else {
                ICON_EXPANDED
            };
            let mut header_spans = vec![Span::styled(
                format!("    {} {}", collapse_indicator, hunk.header.raw),
                header_style,
            )];

            if is_collapsed
                && search_active
                && let Some(count) = matches_by_hunk.get(&idx).copied().filter(|c| *c > 0)
            {
                header_spans.push(Span::raw(" "));
                header_spans.push(Span::styled(
                    format!("(+{} matches)", count),
                    Style::default().fg(self.config.theme.search_match_active),
                ));
            }

            items.push(ListItem::new(Line::from(header_spans)));

            // Only show diff lines if not collapsed
            if !is_collapsed {
                // Create a fresh highlighter for this hunk to prevent state leakage
                let mut highlighter = render_context.create_fresh_highlighter();

                // Diff lines with deeper indentation - uses DiffRenderer with automatic syntax highlighting
                for (line_index, line) in hunk.lines.iter().enumerate() {
                    let highlights = matches_by_line.get(&(idx, line_index));
                    let has_active_match = highlights
                        .map(|list| list.iter().any(|h| h.is_active))
                        .unwrap_or(false);

                    let formatted_line = render_context.format_diff_line(
                        line,
                        is_selected,
                        Some("      "),
                        highlighter.as_mut(),
                        highlights.map(|h| h.as_slice()),
                    );
                    if search_active && is_search_context && has_active_match {
                        render_ctx.ensure_visible(items.len());
                    }
                    items.push(ListItem::new(formatted_line));
                }
            }

            if diff.hunks.len() > 1 {
                items.push(ListItem::new(Line::from("")));
            }

            let end_line = items.len();
            overlays.push(HunkOverlay {
                start_line,
                end_line,
                section_label: section_label(section),
                file_path: diff.file_path.clone(),
                header: hunk.header.raw.clone(),
                selected: is_selected,
            });
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
                .diff_hunk_header
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

    fn render_sticky_overlay(&self, f: &mut Frame, area: Rect, overlay: &HunkOverlay) {
        let spans = vec![
            Span::styled(
                overlay.section_label.clone(),
                self.config.theme.section_header,
            ),
            Span::raw(" • "),
            Span::styled(
                overlay.file_path.clone(),
                Style::default().fg(self.config.theme.selected_fg),
            ),
            Span::raw(" • "),
            Span::styled(overlay.header.clone(), self.config.theme.diff_hunk_header),
        ];

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line);
        f.render_widget(paragraph, area);
    }
}

fn ranges_intersect(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start < b_end && b_start < a_end
}

fn section_label(section: StatusSection) -> String {
    match section {
        StatusSection::Staged => "Staged changes".to_string(),
        StatusSection::Unstaged => "Unstaged changes".to_string(),
        StatusSection::Untracked => "Untracked files".to_string(),
        StatusSection::Conflicted => "Conflicted files".to_string(),
    }
}
