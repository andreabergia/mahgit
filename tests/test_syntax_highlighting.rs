use mahgit::config::Config;
use mahgit::diff::{Diff, DiffContext, DiffHunk, DiffLine, HunkHeader, LineRange, LineType};
use mahgit::ui::diff_renderer::DiffRenderer;

/// Helper to create a Rust diff for testing
fn create_rust_diff() -> Diff {
    let hunk = DiffHunk {
        header: HunkHeader {
            raw: "@@ -1,3 +1,3 @@".to_string(),
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 3,
        },
        old_range: LineRange { start: 1, count: 3 },
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
                line_type: LineType::Deletion,
                old_line_no: Some(2),
                new_line_no: None,
                inline_diff: None,
            },
            DiffLine {
                content: "    let x = 43;".to_string(),
                line_type: LineType::Addition,
                old_line_no: None,
                new_line_no: Some(2),
                inline_diff: None,
            },
            DiffLine {
                content: "}".to_string(),
                line_type: LineType::Context,
                old_line_no: Some(3),
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
fn test_syntax_highlighting_produces_multiple_colors() {
    let config = Config {
        theme: mahgit::theme::Theme::default(),
        tab_width: 4,
        show_line_numbers: false,
    };

    let renderer = DiffRenderer::new(&config);
    let diff = create_rust_diff();

    let lines = renderer.generate_diff_lines(&diff, Some(0), None);

    // Skip the header line
    let content_lines = &lines[1..];

    println!("\n=== Analyzing rendered lines ===");
    for (i, line) in content_lines.iter().enumerate() {
        println!("\nLine {}: {} spans", i, line.spans.len());

        // Collect unique foreground colors (excluding gutter and prefix)
        let mut unique_colors = std::collections::HashSet::new();
        for (j, span) in line.spans.iter().enumerate() {
            if let Some(fg) = span.style.fg {
                unique_colors.insert(format!("{:?}", fg));
                println!("  Span {}: '{}' fg={:?}", j, span.content, fg);
            } else {
                println!("  Span {}: '{}' fg=None", j, span.content);
            }
        }

        println!("  Unique colors: {}", unique_colors.len());

        // For Rust code lines with keywords and identifiers, we should see multiple colors
        // The "fn main() {" line should have:
        // - gutter color
        // - space
        // - diff prefix (+ or - or space)
        // - "fn" in one color (keyword)
        // - " " in another color
        // - "main" possibly in another color (function name)
        // - "() {" in base color
        if line.spans.iter().any(|s| s.content.contains("fn main")) {
            println!("  -> This is the 'fn main()' line");

            // We should have multiple spans with content
            let content_spans: Vec<_> = line
                .spans
                .iter()
                .filter(|s| !s.content.is_empty() && s.content != "|" && s.content != " ")
                .collect();

            println!("  -> Content spans: {}", content_spans.len());

            // With syntax highlighting, "fn main() {" should be split into multiple spans
            // with different colors for keywords vs identifiers
            assert!(
                content_spans.len() >= 3,
                "Expected at least 3 content spans for 'fn main() {{' with syntax highlighting, got {}",
                content_spans.len()
            );
        }
    }

    // Check that we have at least some color variation
    let mut all_fg_colors = std::collections::HashSet::new();
    for line in content_lines {
        for span in &line.spans {
            if let Some(fg) = span.style.fg {
                all_fg_colors.insert(format!("{:?}", fg));
            }
        }
    }

    println!("\n=== Summary ===");
    println!("Total unique foreground colors: {}", all_fg_colors.len());
    for color in &all_fg_colors {
        println!("  - {}", color);
    }

    assert!(
        all_fg_colors.len() >= 4,
        "Expected at least 4 different colors with syntax highlighting (gutter, keywords, identifiers, etc.), got {}",
        all_fg_colors.len()
    );
}

#[test]
fn test_syntax_highlighter_actually_highlights() {
    use mahgit::diff::SyntaxHighlighter;

    let highlighter = SyntaxHighlighter::new();

    // Test that we can detect Rust syntax
    let syntax = highlighter.detect_syntax("test.rs");
    assert!(syntax.is_some(), "Should detect Rust syntax for .rs files");

    let syntax_ref = syntax.unwrap();
    println!("\nDetected syntax: {}", syntax_ref.name);
    assert_eq!(syntax_ref.name, "Rust");

    // Test that highlighting produces multiple colored segments
    let mut hl = highlighter.create_highlighter(syntax_ref);
    let line = "fn main() {";
    let highlighted = highlighter.highlight_line(line, syntax_ref, &mut hl);

    println!("\n=== Syntax highlighting for 'fn main() {{' ===");
    for (text, color) in &highlighted {
        println!("  '{}' -> {:?}", text, color);
    }

    // Should have multiple segments with different colors
    assert!(
        highlighted.len() >= 3,
        "Expected at least 3 segments from syntax highlighting 'fn main() {{', got {}",
        highlighted.len()
    );

    // Collect unique colors
    let unique_colors: std::collections::HashSet<_> = highlighted
        .iter()
        .map(|(_, c)| format!("{:?}", c))
        .collect();

    println!("Unique colors: {}", unique_colors.len());
    for color in &unique_colors {
        println!("  - {}", color);
    }

    assert!(
        unique_colors.len() >= 2,
        "Expected at least 2 different colors for 'fn main() {{', got {}",
        unique_colors.len()
    );
}

#[test]
fn test_context_lines_have_syntax_highlighting() {
    let config = Config {
        theme: mahgit::theme::Theme::default(),
        tab_width: 4,
        show_line_numbers: false,
    };

    let renderer = DiffRenderer::new(&config);
    let diff = create_rust_diff();

    let lines = renderer.generate_diff_lines(&diff, None, None);

    // Find a context line (the "fn main() {" line)
    let context_line = lines
        .iter()
        .find(|line| line.spans.iter().any(|s| s.content.contains("fn main")))
        .expect("Should have a line with 'fn main'");

    println!("\n=== Context line analysis ===");
    println!("Number of spans: {}", context_line.spans.len());

    for (i, span) in context_line.spans.iter().enumerate() {
        println!(
            "Span {}: '{}' fg={:?} bg={:?}",
            i, span.content, span.style.fg, span.style.bg
        );
    }

    // The context line should have syntax highlighting
    // Look for spans with actual code content (not gutter/prefix)
    let code_spans: Vec<_> = context_line
        .spans
        .iter()
        .filter(|s| {
            !s.content.is_empty()
                && s.content != "|"
                && s.content != " "
                && !s
                    .content
                    .chars()
                    .all(|c| c.is_numeric() || c.is_whitespace())
        })
        .collect();

    println!("Code spans: {}", code_spans.len());

    // With syntax highlighting, the code should be split into multiple colored spans
    assert!(
        code_spans.len() >= 2,
        "Expected multiple spans with syntax highlighting for context line, got {}",
        code_spans.len()
    );
}

#[test]
fn test_inactive_context_lines_still_have_syntax_colors() {
    let config = Config {
        theme: mahgit::theme::Theme::default(),
        tab_width: 4,
        show_line_numbers: false,
    };

    let renderer = DiffRenderer::new(&config);
    let diff = create_rust_diff();

    // Render with NO active hunk (all lines should be dimmed)
    let lines = renderer.generate_diff_lines(&diff, None, None);

    // Find the "fn main()" context line
    let fn_main_line = lines
        .iter()
        .find(|line| line.spans.iter().any(|s| s.content.contains("fn main")))
        .expect("Should have 'fn main' line");

    println!("\n=== Inactive context line 'fn main()' ===");
    println!("Number of spans: {}", fn_main_line.spans.len());

    let mut has_keyword_color = false;
    let mut has_function_color = false;

    for (i, span) in fn_main_line.spans.iter().enumerate() {
        println!(
            "Span {}: '{}' fg={:?} mods={:?}",
            i, span.content, span.style.fg, span.style.add_modifier
        );

        // Check if we have colored spans for code
        if span.content == "fn" && span.style.fg.is_some() {
            has_keyword_color = true;
            // The span itself should NOT have DIM modifier
            // (DIM was only on the base content_style which isn't used for syntax spans)
            println!("  -> Found 'fn' keyword with color!");
        }
        if span.content == "main" && span.style.fg.is_some() {
            has_function_color = true;
            println!("  -> Found 'main' function with color!");
        }
    }

    assert!(
        has_keyword_color,
        "Expected 'fn' keyword to have syntax highlighting color even in inactive hunk"
    );
    assert!(
        has_function_color,
        "Expected 'main' function to have syntax highlighting color even in inactive hunk"
    );
}

#[test]
fn test_syntax_detection_for_rust_file() {
    use mahgit::diff::SyntaxHighlighter;

    let highlighter = SyntaxHighlighter::new();

    // Test various Rust file paths
    let test_paths = vec!["test.rs", "src/main.rs", "lib.rs", "src/diff/syntax.rs"];

    for path in test_paths {
        let syntax = highlighter.detect_syntax(path);
        assert!(
            syntax.is_some(),
            "Should detect Rust syntax for path: {}",
            path
        );
        assert_eq!(syntax.unwrap().name, "Rust", "Path: {}", path);
    }
}
