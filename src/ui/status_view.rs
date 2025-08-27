use crate::status::RepositoryStatus;
use crate::ui::navigation::{NavigationState, StatusSection};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
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

        if self.status.is_clean() {
            let clean_text = Text::from("Working directory is clean");
            let paragraph = Paragraph::new(clean_text);
            f.render_widget(paragraph, inner_area);
        } else {
            self.render_file_sections(f, inner_area);
        }
    }

    fn render_file_sections(&self, f: &mut Frame, area: Rect) {
        let mut items = Vec::new();
        let mut current_line = 0;
        let selected_global_index = self.navigation.get_global_index();

        self.add_section_items(
            &mut items,
            &mut current_line,
            StatusSection::Staged,
            "Staged changes",
            self.status.staged_files(),
            selected_global_index,
        );

        self.add_section_items(
            &mut items,
            &mut current_line,
            StatusSection::Unstaged,
            "Unstaged changes",
            self.status.unstaged_files(),
            selected_global_index,
        );

        self.add_section_items(
            &mut items,
            &mut current_line,
            StatusSection::Untracked,
            "Untracked files",
            self.status.untracked_files(),
            selected_global_index,
        );

        self.add_section_items(
            &mut items,
            &mut current_line,
            StatusSection::Conflicted,
            "Conflicted files",
            self.status.conflicted_files(),
            selected_global_index,
        );

        let list = List::new(items);
        f.render_widget(list, area);
    }

    fn add_section_items(
        &self,
        items: &mut Vec<ListItem>,
        current_line: &mut usize,
        section: StatusSection,
        header: &str,
        files: &[String],
        selected_global_index: usize,
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
        *current_line += 1;

        for file in files.iter() {
            let file_indicator = self.get_file_indicator(section);
            let content = format!("  {} {}", file_indicator, file);

            let is_selected = *current_line == selected_global_index;
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
            *current_line += 1;
        }

        items.push(ListItem::new(Line::from("")));
        *current_line += 1;
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
