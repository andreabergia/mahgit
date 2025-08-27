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
    current_view: ViewType,
    status: RepositoryStatus,
    navigation: NavigationState,
    input_handler: InputHandler,
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
        }
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
                // TODO: Implement help display
            }
            Command::Unknown => {
                // Ignore unknown commands
            }
        }
    }

    fn render(&self, f: &mut ratatui::Frame) {
        let status_view = StatusView::new(&self.status, &self.navigation);
        status_view.render(f, f.area());
    }
}
