use mahgit::config::Config;
use mahgit::diff::{
    Diff, DiffContext, DiffHunk, DiffLine, HunkHeader, LineRange, LineType, SyntaxHighlighter,
};
use mahgit::theme::Theme;
use mahgit::ui::diff_renderer::{DiffRenderContext, DiffRenderer};
use ratatui::style::Color;

fn create_rust_diff() -> Diff {
    let hunk = DiffHunk {
        header: HunkHeader {
            raw: "@@ -1,1 +1,3 @@".to_string(),
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 3,
        },
        old_range: LineRange { start: 1, count: 1 },
        new_range: LineRange { start: 1, count: 3 },
        stageable: true,
        context_lines: 3,
        lines: vec![
            DiffLine {
                content: "fn main() {".to_string(),
                line_type: LineType::Context,
                old_line_no: Some(1),
                new_line_no: Some(1),
                inline_diff: None,
            },
            DiffLine {
                content: "    let x = 42;".to_string(),
                line_type: LineType::Addition,
                old_line_no: None,
                new_line_no: Some(2),
                inline_diff: None,
            },
            DiffLine {
                content: "    println!(\"Hello {}\", x);".to_string(),
                line_type: LineType::Addition,
                old_line_no: None,
                new_line_no: Some(3),
                inline_diff: None,
            },
        ],
    };

    Diff {
        file_path: "test.rs".to_string(),
        context: DiffContext::WorkingTreeToIndex,
        hunks: vec![hunk],
        binary: false,
    }
}

#[test]
fn test_syntax_highlighter_basic() {
    println!("\n=== TEST 1: Basic SyntaxHighlighter ===");
    let highlighter = SyntaxHighlighter::new();
    println!("✓ SyntaxHighlighter created");

    let syntax = highlighter.detect_syntax("test.rs");
    assert!(syntax.is_some(), "Should detect Rust syntax");
    println!("✓ Syntax detected: {}", syntax.unwrap().name);
}

#[test]
fn test_syntax_highlighter_produces_colored_spans() {
    println!("\n=== TEST 2: Syntax highlighting produces colors ===");
    let highlighter = SyntaxHighlighter::new();
    let syntax = highlighter.detect_syntax("test.rs").unwrap();
    let mut hl = highlighter.create_highlighter(syntax);

    let line = "fn main() {";
    let spans = highlighter.highlight_line(line, syntax, &mut hl);

    println!("Input: '{}'", line);
    println!("Output: {} spans", spans.len());
    for (i, (text, color)) in spans.iter().enumerate() {
        println!("  Span {}: '{}' -> {:?}", i, text, color);
    }

    assert!(spans.len() > 1, "Should have multiple spans");

    // Check that we have different colors
    let colors: Vec<_> = spans.iter().map(|(_, c)| c).collect();
    let unique_colors: std::collections::HashSet<_> = colors.iter().cloned().collect();
    assert!(unique_colors.len() > 1, "Should have different colors");
    println!("✓ Found {} unique colors", unique_colors.len());
}

#[test]
fn test_diff_renderer_has_syntax_highlighter() {
    println!("\n=== TEST 3: DiffRenderer initialization ===");
    let config = Config {
        theme: Theme::default(),
        tab_width: 4,
        show_line_numbers: false,
    };

    let renderer = DiffRenderer::new(&config);
    println!("✓ DiffRenderer created");

    // We can't directly check if syntax_highlighter is Some, but we can test it works
    let diff = create_rust_diff();
    let lines = renderer.generate_diff_lines(&diff, Some(0), None);

    println!("Generated {} lines", lines.len());
    assert!(!lines.is_empty(), "Should generate lines");
}

#[test]
fn test_render_context_detects_syntax() {
    println!("\n=== TEST 4: DiffRenderContext syntax detection ===");
    let config = Config {
        theme: Theme::default(),
        tab_width: 4,
        show_line_numbers: false,
    };
    let renderer = DiffRenderer::new(&config);
    let diff = create_rust_diff();

    let _context = DiffRenderContext::new(&renderer, &diff);
    println!("✓ DiffRenderContext created for test.rs");

    // The context should have detected Rust syntax internally
    // We can't check this directly, but we can test by formatting a line
}

#[test]
fn test_formatted_line_has_syntax_colors() {
    println!("\n=== TEST 5: Formatted line contains syntax colors ===");
    let config = Config {
        theme: Theme::default(),
        tab_width: 4,
        show_line_numbers: false,
    };
    let renderer = DiffRenderer::new(&config);
    let diff = create_rust_diff();
    let context = DiffRenderContext::new(&renderer, &diff);

    // Create a fresh highlighter for this hunk
    let mut highlighter = context.create_fresh_highlighter();

    // Format the first content line: "fn main() {"
    let line = &diff.hunks[0].lines[0];
    let formatted = context.format_diff_line(line, true, None, highlighter.as_mut(), None);

    println!("Original: '{}'", line.content);
    println!("Formatted into {} spans:", formatted.spans.len());

    let mut has_syntax_color = false;
    for (i, span) in formatted.spans.iter().enumerate() {
        println!(
            "  Span {}: '{}' fg={:?} bg={:?}",
            i, span.content, span.style.fg, span.style.bg
        );

        // Check if this span has a foreground color that's not the default theme colors
        if let Some(fg) = span.style.fg {
            match fg {
                Color::LightMagenta
                | Color::LightCyan
                | Color::LightYellow
                | Color::Yellow
                | Color::DarkGray => {
                    has_syntax_color = true;
                    println!("    ^ This is a syntax color!");
                }
                Color::Rgb(_, _, _) => {
                    // Could be syntax color
                    has_syntax_color = true;
                    println!("    ^ This is a syntax RGB color!");
                }
                _ => {}
            }
        }
    }

    if !has_syntax_color {
        println!("\n⚠️  WARNING: No syntax colors found in formatted line!");
        println!("Expected to see LightMagenta, LightCyan, LightYellow, or RGB colors");
    }

    assert!(
        has_syntax_color,
        "Formatted line should have syntax highlighting colors"
    );
}

#[test]
fn test_addition_line_has_syntax_colors() {
    println!("\n=== TEST 6: Addition line with syntax colors ===");
    let config = Config {
        theme: Theme::default(),
        tab_width: 4,
        show_line_numbers: false,
    };
    let renderer = DiffRenderer::new(&config);
    let diff = create_rust_diff();
    let context = DiffRenderContext::new(&renderer, &diff);

    // Create a fresh highlighter for this hunk
    let mut highlighter = context.create_fresh_highlighter();

    // Format an addition line: "    let x = 42;"
    let line = &diff.hunks[0].lines[1];
    assert_eq!(line.line_type, LineType::Addition);

    let formatted = context.format_diff_line(line, true, None, highlighter.as_mut(), None);

    println!("Original: '{}'", line.content);
    println!("Line type: Addition (+)");
    println!("Formatted into {} spans:", formatted.spans.len());

    for (i, span) in formatted.spans.iter().enumerate() {
        println!(
            "  Span {}: '{}' fg={:?} bg={:?}",
            i, span.content, span.style.fg, span.style.bg
        );
    }

    // Check that we have syntax colors, not just the green addition color
    let syntax_colors = formatted
        .spans
        .iter()
        .filter(|span| {
            if let Some(fg) = span.style.fg {
                matches!(
                    fg,
                    Color::LightMagenta
                        | Color::LightCyan
                        | Color::LightYellow
                        | Color::Yellow
                        | Color::DarkGray
                        | Color::Rgb(_, _, _)
                )
            } else {
                false
            }
        })
        .count();

    println!("\nFound {} spans with syntax colors", syntax_colors);
    assert!(
        syntax_colors > 0,
        "Addition line should have syntax highlighting"
    );
}

#[test]
fn test_full_diff_rendering() {
    println!("\n=== TEST 7: Full diff rendering ===");
    let config = Config {
        theme: Theme::default(),
        tab_width: 4,
        show_line_numbers: false,
    };
    let renderer = DiffRenderer::new(&config);
    let diff = create_rust_diff();

    let lines = renderer.generate_diff_lines(&diff, Some(0), None);

    println!("Generated {} lines for diff", lines.len());

    // Skip the header line
    for (i, line) in lines.iter().skip(1).enumerate() {
        println!("\nLine {}:", i);
        println!("  {} spans:", line.spans.len());
        for (j, span) in line.spans.iter().enumerate() {
            if let Some(fg) = span.style.fg {
                println!("    Span {}: '{}' -> {:?}", j, span.content, fg);
            }
        }
    }

    // Count lines with syntax colors
    let mut lines_with_syntax = 0;
    for line in lines.iter().skip(1) {
        let has_syntax = line.spans.iter().any(|span| {
            span.style
                .fg
                .map(|fg| {
                    matches!(
                        fg,
                        Color::LightMagenta
                            | Color::LightCyan
                            | Color::LightYellow
                            | Color::Yellow
                            | Color::DarkGray
                            | Color::Rgb(_, _, _)
                    )
                })
                .unwrap_or(false)
        });
        if has_syntax {
            lines_with_syntax += 1;
        }
    }

    println!(
        "\n{}/{} content lines have syntax highlighting",
        lines_with_syntax,
        lines.len() - 1
    );
    assert!(
        lines_with_syntax > 0,
        "At least some lines should have syntax highlighting"
    );
}
