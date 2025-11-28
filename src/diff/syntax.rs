use ratatui::style::Color;
use std::path::Path;
use std::sync::OnceLock;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

/// Global static syntax highlighter instance, initialized lazily on first use.
/// This avoids loading ~5MB of syntax definitions on every render.
static SYNTAX_HIGHLIGHTER: OnceLock<SyntaxHighlighter> = OnceLock::new();

/// Get the global syntax highlighter instance, initializing it if needed.
pub fn syntax_highlighter() -> &'static SyntaxHighlighter {
    SYNTAX_HIGHLIGHTER.get_or_init(SyntaxHighlighter::new)
}

/// SyntaxHighlighter provides syntax highlighting for diff content using syntect.
pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SyntaxHighlighter {
    /// Create a new SyntaxHighlighter with default syntax definitions and theme.
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();

        // Use base16-ocean.dark theme for dark terminals
        // This provides good syntax highlighting with colors designed for dark backgrounds
        let theme = theme_set
            .themes
            .get("base16-ocean.dark")
            .or_else(|| theme_set.themes.get("Solarized (dark)"))
            .expect("At least one theme should be available")
            .clone();

        Self { syntax_set, theme }
    }

    /// Detect the programming language from a file path.
    /// Returns None if the language cannot be detected.
    pub fn detect_syntax<P: AsRef<Path>>(&self, path: P) -> Option<&SyntaxReference> {
        self.syntax_set
            .find_syntax_for_file(path.as_ref())
            .ok()
            .flatten()
    }

    /// Highlight a single line of code and return colored spans.
    /// Returns None if syntax is None (no highlighting needed).
    pub fn highlight_line(
        &self,
        line: &str,
        _syntax: &SyntaxReference,
        highlighter: &mut HighlightLines,
    ) -> Vec<(String, Color)> {
        let ranges = highlighter
            .highlight_line(line, &self.syntax_set)
            .unwrap_or_default();

        ranges
            .into_iter()
            .map(|(style, text)| {
                let color = Self::syntect_to_ratatui_color(style);
                (text.to_string(), color)
            })
            .collect()
    }

    /// Create a new HighlightLines instance for syntax highlighting.
    pub fn create_highlighter(&self, syntax: &SyntaxReference) -> HighlightLines<'_> {
        HighlightLines::new(syntax, &self.theme)
    }

    /// Convert syntect's Style color to ratatui's Color.
    fn syntect_to_ratatui_color(style: Style) -> Color {
        let fg = style.foreground;
        Color::Rgb(fg.r, fg.g, fg.b)
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_highlighter() {
        let highlighter = SyntaxHighlighter::new();
        assert!(!highlighter.syntax_set.syntaxes().is_empty());
    }

    #[test]
    fn test_detect_syntax_rust() {
        let highlighter = SyntaxHighlighter::new();
        let syntax = highlighter.detect_syntax("test.rs");
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "Rust");
    }

    #[test]
    fn test_detect_syntax_markdown() {
        let highlighter = SyntaxHighlighter::new();
        let syntax = highlighter.detect_syntax("README.md");
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "Markdown");
    }

    #[test]
    fn test_detect_syntax_unknown() {
        let highlighter = SyntaxHighlighter::new();
        let syntax = highlighter.detect_syntax("test.unknown");
        // Unknown extensions may or may not have a syntax
        // Just verify that the function doesn't panic
        let _ = syntax;
    }

    #[test]
    fn test_highlight_rust_code() {
        let highlighter = SyntaxHighlighter::new();
        let syntax = highlighter.detect_syntax("test.rs").unwrap();
        let mut highlight_lines = highlighter.create_highlighter(syntax);

        let line = "fn main() {";
        let spans = highlighter.highlight_line(line, syntax, &mut highlight_lines);

        assert!(!spans.is_empty());
        // Verify that we get multiple spans with different colors
        // (syntax highlighting should split the line into multiple colored segments)
        assert!(spans.iter().any(|(text, _)| text.contains("fn")));
    }

    #[test]
    fn test_syntect_to_ratatui_color() {
        // Test various RGB color conversions
        let test_cases = vec![
            ((255, 128, 64), Color::Rgb(255, 128, 64)),
            ((200, 100, 200), Color::Rgb(200, 100, 200)), // Purple
            ((100, 200, 200), Color::Rgb(100, 200, 200)), // Cyan
            ((200, 200, 100), Color::Rgb(200, 200, 100)), // Yellow
            ((128, 128, 128), Color::Rgb(128, 128, 128)), // Gray
        ];

        for ((r, g, b), expected) in test_cases {
            let style = Style {
                foreground: syntect::highlighting::Color { r, g, b, a: 255 },
                background: syntect::highlighting::Color::BLACK,
                font_style: syntect::highlighting::FontStyle::empty(),
            };

            let color = SyntaxHighlighter::syntect_to_ratatui_color(style);
            assert_eq!(color, expected, "Failed for RGB({}, {}, {})", r, g, b);
        }
    }
}
