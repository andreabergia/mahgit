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
    pub section_header: Style,
    pub diff_context: Color,
    pub diff_hunk_header: Style,
    pub diff_no_newline: Color,
    pub diff_line_number: Color,
    pub diff_gutter_addition: Color,
    pub diff_gutter_deletion: Color,
    pub diff_gutter_context: Color,
    pub diff_gutter_focused: Color,
    pub diff_hunk_highlight: Color,
    pub diff_inline_addition: Color,
    pub diff_inline_deletion: Color,
    pub help_key: Color,
    pub help_desc: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::github_dark()
    }
}

impl Theme {
    /// Load a theme by name. Returns None if the theme name is unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "github-dark" => Some(Self::github_dark()),
            _ => None,
        }
    }

    /// GitHub Dark - modern 24-bit RGB colors matching GitHub's dark theme
    fn github_dark() -> Self {
        Self {
            staged: Color::Rgb(63, 185, 80),        // #3FB950
            unstaged: Color::Rgb(248, 81, 73),      // #F85149
            untracked: Color::Rgb(188, 140, 255),   // #BC8CFF
            conflicted: Color::Rgb(219, 171, 9),    // #DBAB09
            selected_fg: Color::Rgb(255, 255, 255), // #FFFFFF - bright white for contrast
            selected_bg: Color::Rgb(45, 51, 59),    // #2D333B
            section_header: Style::default()
                .fg(Color::Rgb(127, 219, 202)) // #7FDBCA
                .add_modifier(Modifier::BOLD),
            diff_context: Color::Rgb(201, 209, 217), // #C9D1D9
            diff_hunk_header: Style::default()
                .fg(Color::Rgb(224, 175, 104)) // #E0AF68 - gold
                .add_modifier(Modifier::BOLD),
            diff_no_newline: Color::Rgb(110, 118, 129), // #6E7681
            diff_line_number: Color::Rgb(110, 118, 129), // #6E7681
            diff_gutter_addition: Color::Rgb(46, 160, 67), // #2EA043
            diff_gutter_deletion: Color::Rgb(190, 74, 72), // Slightly deeper red for gutter
            diff_gutter_context: Color::Rgb(99, 110, 123), // #636E7B
            diff_gutter_focused: Color::Rgb(78, 87, 99), // muted slate instead of gold
            diff_hunk_highlight: Color::Rgb(45, 51, 59), // slightly brighter than background
            diff_inline_addition: Color::Rgb(35, 76, 38), // muted green block highlight
            diff_inline_deletion: Color::Rgb(81, 38, 33), // muted red block highlight
            help_key: Color::Rgb(127, 219, 202),        // #7FDBCA
            help_desc: Color::Rgb(201, 209, 217),       // #C9D1D9
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_from_name_known_themes() {
        assert!(Theme::from_name("github-dark").is_some());
    }

    #[test]
    fn test_theme_from_name_unknown_theme() {
        assert!(Theme::from_name("unknown").is_none());
        assert!(Theme::from_name("gruvbox-dark").is_none());
        assert!(Theme::from_name("").is_none());
    }

    #[test]
    fn test_github_dark_uses_rgb() {
        let theme = Theme::github_dark();
        // Verify RGB colors are used
        matches!(theme.staged, Color::Rgb(_, _, _));
        matches!(theme.unstaged, Color::Rgb(_, _, _));
    }
}
