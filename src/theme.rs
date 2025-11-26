use ratatui::style::{Color, Modifier, Style};

/// Theme defines all colors and styles used throughout the application.
#[derive(Debug, Clone)]
pub struct Theme {
    pub staged: Color,
    pub unstaged: Color,
    pub untracked: Color,
    pub conflicted: Color,
    pub selected_fg: Color,
    pub selected_bg: Color,
    pub selected_hunk_bg: Option<Color>,
    pub section_header: Style,
    pub diff_context: Color,
    pub diff_hunk_header: Style,
    pub diff_hunk_header_focused: Style,
    pub diff_no_newline: Color,
    pub help_key: Color,
    pub help_desc: Color,
}

impl Theme {
    /// Load a theme by name. Returns None if the theme name is unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "gruvbox-dark" => Some(Self::gruvbox_dark()),
            "gruvbox-light" => Some(Self::gruvbox_light()),
            "github-dark" => Some(Self::github_dark()),
            "github-light" => Some(Self::github_light()),
            _ => None,
        }
    }

    /// Gruvbox Dark - default theme using 16 ANSI colors
    fn gruvbox_dark() -> Self {
        Self {
            staged: Color::Green,
            unstaged: Color::Red,
            untracked: Color::Magenta,
            conflicted: Color::Yellow,
            selected_fg: Color::White,
            selected_bg: Color::DarkGray,
            selected_hunk_bg: None,
            section_header: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            diff_context: Color::White,
            diff_hunk_header: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            diff_hunk_header_focused: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            diff_no_newline: Color::DarkGray,
            help_key: Color::Cyan,
            help_desc: Color::White,
        }
    }

    /// Gruvbox Light - light background variant using 16 ANSI colors
    fn gruvbox_light() -> Self {
        Self {
            staged: Color::Green,
            unstaged: Color::Red,
            untracked: Color::Magenta,
            conflicted: Color::Yellow,
            selected_fg: Color::Black,
            selected_bg: Color::LightBlue,
            selected_hunk_bg: None,
            section_header: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            diff_context: Color::Black,
            diff_hunk_header: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            diff_hunk_header_focused: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            diff_no_newline: Color::Gray,
            help_key: Color::Blue,
            help_desc: Color::Black,
        }
    }

    /// GitHub Dark - modern 24-bit RGB colors matching GitHub's dark theme
    fn github_dark() -> Self {
        Self {
            staged: Color::Rgb(63, 185, 80),                // #3FB950
            unstaged: Color::Rgb(248, 81, 73),              // #F85149
            untracked: Color::Rgb(188, 140, 255),           // #BC8CFF
            conflicted: Color::Rgb(219, 171, 9),            // #DBAB09
            selected_fg: Color::Rgb(201, 209, 217),         // #C9D1D9
            selected_bg: Color::Rgb(45, 51, 59),            // #2D333B
            selected_hunk_bg: Some(Color::Rgb(35, 39, 46)), // #23272E - subtle darker than selected_bg
            section_header: Style::default()
                .fg(Color::Rgb(127, 219, 202)) // #7FDBCA
                .add_modifier(Modifier::BOLD),
            diff_context: Color::Rgb(201, 209, 217), // #C9D1D9
            diff_hunk_header: Style::default()
                .fg(Color::Rgb(127, 219, 202)) // #7FDBCA
                .add_modifier(Modifier::BOLD),
            diff_hunk_header_focused: Style::default()
                .fg(Color::Rgb(224, 175, 104)) // #E0AF68
                .add_modifier(Modifier::BOLD),
            diff_no_newline: Color::Rgb(110, 118, 129), // #6E7681
            help_key: Color::Rgb(127, 219, 202),        // #7FDBCA
            help_desc: Color::Rgb(201, 209, 217),       // #C9D1D9
        }
    }

    /// GitHub Light - light variant matching GitHub's light theme
    fn github_light() -> Self {
        Self {
            staged: Color::Rgb(31, 136, 61),                   // #1F883D
            unstaged: Color::Rgb(207, 34, 46),                 // #CF222E
            untracked: Color::Rgb(130, 80, 223),               // #8250DF
            conflicted: Color::Rgb(191, 135, 0),               // #BF8700
            selected_fg: Color::Rgb(36, 41, 47),               // #24292F
            selected_bg: Color::Rgb(208, 215, 222),            // #D0D7DE
            selected_hunk_bg: Some(Color::Rgb(234, 238, 242)), // #EAEEF2 - subtle lighter than selected_bg
            section_header: Style::default()
                .fg(Color::Rgb(9, 105, 218)) // #0969DA
                .add_modifier(Modifier::BOLD),
            diff_context: Color::Rgb(36, 41, 47), // #24292F
            diff_hunk_header: Style::default()
                .fg(Color::Rgb(9, 105, 218)) // #0969DA
                .add_modifier(Modifier::BOLD),
            diff_hunk_header_focused: Style::default()
                .fg(Color::Rgb(130, 80, 223)) // #8250DF
                .add_modifier(Modifier::BOLD),
            diff_no_newline: Color::Rgb(101, 109, 118), // #656D76
            help_key: Color::Rgb(9, 105, 218),          // #0969DA
            help_desc: Color::Rgb(36, 41, 47),          // #24292F
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_from_name_known_themes() {
        assert!(Theme::from_name("gruvbox-dark").is_some());
        assert!(Theme::from_name("gruvbox-light").is_some());
        assert!(Theme::from_name("github-dark").is_some());
        assert!(Theme::from_name("github-light").is_some());
    }

    #[test]
    fn test_theme_from_name_unknown_theme() {
        assert!(Theme::from_name("unknown").is_none());
        assert!(Theme::from_name("solarized").is_none());
        assert!(Theme::from_name("").is_none());
    }

    #[test]
    fn test_gruvbox_dark_theme_colors() {
        let theme = Theme::gruvbox_dark();
        assert_eq!(theme.staged, Color::Green);
        assert_eq!(theme.unstaged, Color::Red);
        assert_eq!(theme.untracked, Color::Magenta);
    }

    #[test]
    fn test_github_dark_uses_rgb() {
        let theme = Theme::github_dark();
        // Verify RGB colors are used
        matches!(theme.staged, Color::Rgb(_, _, _));
        matches!(theme.unstaged, Color::Rgb(_, _, _));
    }
}
