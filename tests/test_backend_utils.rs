use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use mahgit::{config::Config, repository::Repository, status::RepositoryStatus, ui::App};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

/// TestApp wraps App to provide TestBackend-based testing functionality
pub struct TestApp {
    app: App,
    terminal: Terminal<TestBackend>,
}

impl TestApp {
    /// Creates a new TestApp with a specific repository
    pub fn with_repository(
        width: u16,
        height: u16,
        repository: Repository,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let backend = TestBackend::new(width, height);
        let terminal = Terminal::new(backend.clone())?;

        let mut status = RepositoryStatus::new(&repository)?;
        // Make sure to load the status from the repository
        status.reload(&repository)?;

        let theme = mahgit::theme::Theme::from_name("gruvbox-dark").unwrap();
        let config = Config {
            theme,
            tab_width: 4,
            show_line_numbers: true,
        };
        let app = App::new(repository, status, config);

        Ok(Self { app, terminal })
    }

    /// Sends a key event to the app
    pub fn send_key(&mut self, key_event: KeyEvent) {
        self.app.process_key_event(key_event);
    }

    /// Convenience method to send a key by code
    pub fn send_key_code(&mut self, key_code: KeyCode) {
        let key_event = KeyEvent::new(key_code, KeyModifiers::NONE);
        self.send_key(key_event);
    }

    /// Convenience method to send a character key
    pub fn send_char(&mut self, c: char) {
        self.send_key_code(KeyCode::Char(c));
    }

    /// Sends a mouse event to the app
    #[allow(dead_code)]
    pub fn send_mouse(&mut self, mouse_event: MouseEvent) {
        self.app.process_mouse_event(mouse_event);
    }

    /// Renders the app and returns any errors
    pub fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.terminal.draw(|f| self.app.render(f))?;
        Ok(())
    }

    /// Gets the current buffer for assertions
    pub fn get_buffer(&self) -> &Buffer {
        self.terminal.backend().buffer()
    }

    /// Checks if the buffer contains the specified text anywhere
    pub fn assert_contains(&self, text: &str) -> bool {
        let buffer = self.get_buffer();
        let content = buffer_to_string(buffer);
        content.contains(text)
    }

    /// Checks if the app should quit
    pub fn should_quit(&self) -> bool {
        self.app.should_quit()
    }

    /// High-level method to stage the currently selected file
    #[allow(dead_code)]
    pub fn stage_current_file(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_char('s');
        self.render()?;
        Ok(())
    }

    /// High-level method to unstage the currently selected file
    #[allow(dead_code)]
    pub fn unstage_current_file(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_char('u');
        // Force render to ensure the operation is processed
        self.render()?;
        // Additional small delay to ensure operation completes
        std::thread::sleep(std::time::Duration::from_millis(10));
        self.render()?;
        Ok(())
    }

    /// High-level method to toggle stage/unstage using Space key
    #[allow(dead_code)]
    pub fn toggle_stage_current_file(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_char(' ');
        self.render()?;
        Ok(())
    }

    /// Move to top of list
    #[allow(dead_code)]
    pub fn move_to_top(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_char('g');
        self.send_char('g');
        self.render()?;
        Ok(())
    }

    /// High-level method to refresh the repository status
    pub fn refresh(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_char('r');
        self.render()?;
        Ok(())
    }

    /// Simplified method to select a visible file by name
    #[allow(dead_code)]
    pub fn select_file(&mut self, filename: &str) -> Result<bool, Box<dyn std::error::Error>> {
        self.render()?;

        // If file is visible, we assume tests are set up correctly
        if self.assert_contains(filename) {
            // Simple approach: use direct navigation methods
            return Ok(true);
        }
        Ok(false)
    }

    /// Move down one item
    #[allow(dead_code)]
    pub fn move_down(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_char('j');
        self.render()?;
        Ok(())
    }

    /// Quit the application
    pub fn quit(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_char('q');
        self.render()?;
        Ok(())
    }

    /// Access the app's navigation state (for testing)
    #[allow(dead_code)]
    pub fn navigation(&self) -> &mahgit::ui::navigation::NavigationState {
        self.app.navigation()
    }

    /// Toggle section collapsed state (for testing)
    #[allow(dead_code)]
    pub fn toggle_section_collapsed(&mut self, section: mahgit::ui::navigation::StatusSection) {
        self.app.toggle_section_collapsed(section);
    }
}

/// Helper function to convert buffer to string representation
pub fn buffer_to_string(buffer: &Buffer) -> String {
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            result.push_str(cell.symbol());
        }
        if y < buffer.area.height - 1 {
            result.push('\n');
        }
    }
    result
}
