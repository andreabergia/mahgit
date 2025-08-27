pub mod console;
pub mod input;
pub mod navigation;
pub mod status_view;

use crate::status::RepositoryStatus;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use input::{Command, InputHandler};
use navigation::NavigationState;
use ratatui::{Terminal, backend::CrosstermBackend};
use status_view::StatusView;
use std::io::{Stdout, stdout};

pub struct App {
    should_quit: bool,
    #[allow(dead_code)]
    current_view: ViewType,
    status: RepositoryStatus,
    navigation: NavigationState,
    input_handler: InputHandler,
    show_help: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ViewType {
    Status,
}

impl App {
    pub fn new(status: RepositoryStatus) -> Self {
        let navigation = NavigationState::new(&status);
        Self {
            should_quit: false,
            current_view: ViewType::Status,
            status,
            navigation,
            input_handler: InputHandler::new(),
            show_help: false,
        }
    }

    pub fn is_showing_help(&self) -> bool {
        self.show_help
    }

    pub fn process_key_event(&mut self, key_event: crossterm::event::KeyEvent) {
        let command = self.input_handler.handle_key(key_event);
        self.handle_command(command);
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_event_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn run_event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if event::poll(std::time::Duration::from_millis(16))?
                && let Event::Key(key) = event::read()?
            {
                let command = self.input_handler.handle_key(key);
                self.handle_command(command);
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn handle_command(&mut self, command: Command) {
        match command {
            Command::MoveUp => {
                self.navigation.move_up();
            }
            Command::MoveDown => {
                self.navigation.move_down();
            }
            Command::MoveToTop => {
                self.navigation.move_to_top();
            }
            Command::MoveToBottom => {
                self.navigation.move_to_bottom();
            }
            Command::Quit | Command::ForceQuit => {
                self.should_quit = true;
            }
            Command::RefreshStatus => {
                // TODO: Implement status refresh
            }
            Command::ShowHelp => {
                self.show_help = !self.show_help;
            }
            Command::StageFile => {
                // TODO: Implement file staging
            }
            Command::UnstageFile => {
                // TODO: Implement file unstaging
            }
            Command::AddUntracked => {
                // TODO: Implement adding untracked files
            }
            Command::ToggleStage => {
                // TODO: Implement toggle staging
            }
            Command::Unknown => {
                // Ignore unknown commands
            }
        }
    }

    fn render(&self, f: &mut ratatui::Frame) {
        let status_view = StatusView::new(&self.status, &self.navigation);
        status_view.render(f, f.area());

        if self.show_help {
            self.render_help_overlay(f);
        }
    }

    fn render_help_overlay(&self, f: &mut ratatui::Frame) {
        use ratatui::{
            layout::Margin,
            style::Style,
            widgets::{Block, Borders, Clear, List, ListItem},
        };

        let area = f.area();
        let help_text = InputHandler::get_help_text();

        // Calculate height needed for help content (plus borders and title)
        let help_height = (help_text.len() as u16)
            .min(area.height.saturating_sub(2))
            .max(5);

        // Create a bottom panel that slides up from the bottom
        let help_area = ratatui::layout::Rect {
            x: 0,
            y: area.height.saturating_sub(help_height + 2), // +2 for borders
            width: area.width,
            height: help_height + 2,
        };

        // Clear the area where help will be rendered
        f.render_widget(Clear, help_area);

        let help_items: Vec<ListItem> = help_text.into_iter().map(ListItem::new).collect();

        let help_block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT) // No bottom border for slide-up effect
            .title(" Help - Press '?' to close ")
            .style(Style::default()); // No background color, use terminal default

        // Render the block first
        f.render_widget(&help_block, help_area);

        // Calculate the inner area with horizontal margin
        let inner_area = help_block.inner(help_area);
        let content_area = inner_area.inner(Margin {
            horizontal: 1,
            vertical: 0,
        });

        // Render the list content in the margin-adjusted area
        let help_list = List::new(help_items);
        f.render_widget(help_list, content_area);
    }
}
