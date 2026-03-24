use mahgit::config::Config;
use mahgit::diff::{DiffGenerator, SyntaxHighlighter};
use mahgit::repository::Repository;
use mahgit::theme::Theme;
use mahgit::ui::diff_renderer::DiffRenderer;
use ratatui::style::Color;

#[test]
fn test_syntax_highlighting_applied_to_rust_file() {
    // Create a temporary git repository
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();

    // Initialize repository
    let repo = git2::Repository::init(repo_path).unwrap();
    let sig = git2::Signature::now("Test", "test@test.com").unwrap();

    // Create a Rust file
    let file_path = repo_path.join("test.rs");
    std::fs::write(&file_path, "fn main() {\n    println!(\"Hello\");\n}\n").unwrap();

    // Create initial commit
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("test.rs")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .unwrap();

    // Modify the file
    std::fs::write(
        &file_path,
        "fn main() {\n    let x = 42;\n    println!(\"Hello {}\", x);\n}\n",
    )
    .unwrap();

    // Generate diff
    let repository = Repository::open(repo_path).unwrap();
    let git_repo = repository.git2_repo();
    let generator = DiffGenerator::new(git_repo);
    let diff = generator
        .generate_diff("test.rs", mahgit::diff::DiffContext::WorkingTreeToIndex)
        .unwrap();

    // Create renderer with syntax highlighting
    let config = Config {
        theme: Theme::default(),
        tab_width: 4,
        show_line_numbers: false,
        word_wrap: false,
    };
    let renderer = DiffRenderer::new(&config);

    // Generate lines
    let lines = renderer.generate_diff_lines(&diff, Some(0), None);

    // Check that we have multiple spans with different colors (syntax highlighting)
    let mut found_syntax_highlighting = false;
    for line in lines.iter().skip(1) {
        // Skip header
        // Check if line has multiple colored spans (indicating syntax highlighting)
        let colored_spans: Vec<_> = line
            .spans
            .iter()
            .filter(|span| {
                if let Some(fg) = span.style.fg {
                    // Check if it's not just the gutter or default colors
                    !matches!(fg, Color::Reset)
                } else {
                    false
                }
            })
            .collect();

        if colored_spans.len() > 2 {
            // More than just line prefix and content
            found_syntax_highlighting = true;
            println!("\nLine with {} colored spans:", colored_spans.len());
            for span in &line.spans {
                if let Some(fg) = span.style.fg {
                    println!("  '{}' -> {:?}", span.content, fg);
                }
            }
            break;
        }
    }

    assert!(
        found_syntax_highlighting,
        "Should have syntax highlighting with multiple colored spans"
    );
}

#[test]
fn test_syntax_highlighter_detects_rust() {
    let highlighter = SyntaxHighlighter::new();
    let syntax = highlighter.detect_syntax("test.rs");
    assert!(syntax.is_some());
    assert_eq!(syntax.unwrap().name, "Rust");
}

#[test]
fn list_available_themes() {
    use syntect::highlighting::ThemeSet;
    let theme_set = ThemeSet::load_defaults();
    println!("\nAvailable syntect themes:");
    for name in theme_set.themes.keys() {
        println!("  - {}", name);
    }
}

#[test]
fn test_syntax_highlighter_highlights_rust_code() {
    let highlighter = SyntaxHighlighter::new();
    let syntax = highlighter.detect_syntax("test.rs").unwrap();
    let mut hl = highlighter.create_highlighter(syntax);

    let line = "fn main() {";
    let spans = highlighter.highlight_line(line, syntax, &mut hl);

    // Should have multiple spans (fn, main, (), {)
    assert!(
        spans.len() > 1,
        "Should have multiple syntax-highlighted spans"
    );

    // Check that different tokens have different colors
    let colors: Vec<_> = spans.iter().map(|(_, color)| color).collect();
    let unique_colors: std::collections::HashSet<_> = colors.iter().cloned().collect();
    assert!(
        unique_colors.len() > 1,
        "Should have different colors for different tokens"
    );
}
