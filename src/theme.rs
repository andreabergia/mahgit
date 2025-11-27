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
            selected_fg: Color::Black,
            selected_bg: Color::DarkGray,
            section_header: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            diff_context: Color::White,
            diff_hunk_header: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            diff_no_newline: Color::DarkGray,
            diff_line_number: Color::DarkGray,
            diff_gutter_addition: Color::Green,
            diff_gutter_deletion: Color::Red,
            diff_gutter_context: Color::DarkGray,
            diff_gutter_focused: Color::DarkGray,
            diff_hunk_highlight: Color::Gray,
            diff_inline_addition: Color::Rgb(60, 76, 58),
            diff_inline_deletion: Color::Rgb(92, 51, 46),
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
            selected_fg: Color::Blue,
            selected_bg: Color::LightBlue,
            section_header: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            diff_context: Color::Black,
            diff_hunk_header: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            diff_no_newline: Color::Gray,
            diff_line_number: Color::DarkGray,
            diff_gutter_addition: Color::Green,
            diff_gutter_deletion: Color::Red,
            diff_gutter_context: Color::Gray,
            diff_gutter_focused: Color::Gray,
            diff_hunk_highlight: Color::Gray,
            diff_inline_addition: Color::Rgb(213, 238, 214),
            diff_inline_deletion: Color::Rgb(244, 222, 222),
            help_key: Color::Blue,
            help_desc: Color::Black,
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

    /// GitHub Light - light variant matching GitHub's light theme
    fn github_light() -> Self {
        Self {
            staged: Color::Rgb(31, 136, 61),        // #1F883D
            unstaged: Color::Rgb(207, 34, 46),      // #CF222E
            untracked: Color::Rgb(130, 80, 223),    // #8250DF
            conflicted: Color::Rgb(191, 135, 0),    // #BF8700
            selected_fg: Color::Rgb(0, 0, 0),       // #000000 - black for strong contrast
            selected_bg: Color::Rgb(208, 215, 222), // #D0D7DE
            section_header: Style::default()
                .fg(Color::Rgb(9, 105, 218)) // #0969DA
                .add_modifier(Modifier::BOLD),
            diff_context: Color::Rgb(36, 41, 47), // #24292F
            diff_hunk_header: Style::default()
                .fg(Color::Rgb(130, 80, 223)) // #8250DF - purple/magenta
                .add_modifier(Modifier::BOLD),
            diff_no_newline: Color::Rgb(101, 109, 118), // #656D76
            diff_line_number: Color::Rgb(140, 149, 160), // #8C95A0
            diff_gutter_addition: Color::Rgb(46, 160, 67), // #2EA043
            diff_gutter_deletion: Color::Rgb(207, 34, 46), // #CF222E
            diff_gutter_context: Color::Rgb(175, 184, 193), // #AFB8C1
            diff_gutter_focused: Color::Rgb(147, 155, 165), // muted gray
            diff_hunk_highlight: Color::Rgb(224, 227, 231), // softer highlight
            diff_inline_addition: Color::Rgb(218, 251, 225), // #DAFBE1
            diff_inline_deletion: Color::Rgb(255, 235, 233), // #FFEBE9
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
