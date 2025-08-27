use crate::status::RepositoryStatus;
use crate::ui::navigation::{NavigationState, StatusSection};
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

        self.add_section_items(
            &mut items,
            &mut current_file_index,
            StatusSection::Staged,
            "Staged changes",
            self.status.staged_files(),
        );

        self.add_section_items(
            &mut items,
            &mut current_file_index,
            StatusSection::Unstaged,
            "Unstaged changes",
            self.status.unstaged_files(),
        );

        self.add_section_items(
            &mut items,
            &mut current_file_index,
            StatusSection::Untracked,
            "Untracked files",
            self.status.untracked_files(),
        );

        self.add_section_items(
            &mut items,
            &mut current_file_index,
            StatusSection::Conflicted,
            "Conflicted files",
            self.status.conflicted_files(),
        );

        let list = List::new(items);
        f.render_widget(list, area);
    }

    fn add_section_items(
        &self,
        items: &mut Vec<ListItem>,
        current_file_index: &mut usize,
        section: StatusSection,
        header: &str,
        files: &[String],
    ) {
        if files.is_empty() {
            return;
        }

        let section_header = format!("{} ({})", header, files.len());
        let header_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);

        items.push(ListItem::new(Line::from(Span::styled(
            section_header,
            header_style,
        ))));

        for (file_index, file) in files.iter().enumerate() {
            let file_indicator = self.get_file_indicator(section);
            let content = format!("  {} {}", file_indicator, file);

            let is_selected = self.navigation.current_section() == section
                && self.navigation.selected_index() == file_index;

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
        }

        items.push(ListItem::new(Line::from("")));
    }

    fn get_file_indicator(&self, section: StatusSection) -> &'static str {
        match section {
            StatusSection::Staged => "staged",
            StatusSection::Unstaged => "modified",
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
}
