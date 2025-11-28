use mahgit::diff::SyntaxHighlighter;

fn main() {
    let highlighter = SyntaxHighlighter::new();

    // Test detecting Rust syntax
    let syntax = highlighter.detect_syntax("test.rs");
    println!("Syntax detected for test.rs: {:?}", syntax.map(|s| &s.name));

    if let Some(syntax_ref) = syntax {
        let mut hl = highlighter.create_highlighter(syntax_ref);

        // Test highlighting a simple Rust line
        let line = "fn main() {";
        let spans = highlighter.highlight_line(line, syntax_ref, &mut hl);

        println!("\nHighlighting '{}': {} spans", line, spans.len());
        for (i, (text, color)) in spans.iter().enumerate() {
            println!("  Span {}: '{}' -> {:?}", i, text, color);
        }
    }
}
