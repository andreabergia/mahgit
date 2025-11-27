# Syntax Highlighting Implementation Plan

## Overview
Add syntax highlighting to diff content using `syntect`. The implementation will have two phases:
1. **Phase 1**: Simple synchronous implementation to test the feature
2. **Phase 2**: Performance optimization with caching and background processing

## Current Architecture Analysis

### Relevant Files
- `src/ui/diff_view.rs` - Main diff rendering logic (format_diff_line:246-323)
- `src/diff/mod.rs` - Core diff data structures (DiffLine struct)
- `src/ui/inline_diff.rs` - Handles inline diff highlighting (word-level changes)
- `src/theme.rs` - Color theme system
- `Cargo.toml` - Dependencies

### Key Points
- Diff lines are rendered in `format_diff_line()` which creates `Span` objects with colors
- Line content already has tab expansion via `expand_tabs_with_width()`
- Inline diffs use `InlineDiffRenderer` to apply background colors for word changes
- Theme system uses `ratatui::style::Color` (supports RGB)
- `DiffLine` struct contains: content, line_type, line numbers, and optional inline_diff

## Phase 1: Simple Synchronous Implementation

### Goals
- Test the feature with minimal complexity
- Validate the UX and visual appearance
- Ensure syntax highlighting doesn't interfere with existing features (inline diffs, line numbers, gutter)

### Implementation Steps

1. **Add syntect dependency to Cargo.toml**
   - Add `syntect` with default features
   - Version: latest stable (currently ~5.2)

2. **Create syntax highlighting module (`src/diff/syntax.rs`)**
   - Create `SyntaxHighlighter` struct with:
     - `SyntaxSet` and `ThemeSet` from syntect
     - Method to detect language from file path/extension
     - Method to highlight a single line of code
     - Method to convert syntect's `Style` to ratatui's `Color`
   - Handle cases where language detection fails (fallback to no highlighting)
   - Use lazy initialization for `SyntaxSet` (it's expensive to create)

3. **Integrate with DiffView**
   - Add optional `SyntaxHighlighter` field to `DiffView`
   - Initialize it in `DiffView::new()` with a simple theme (e.g., "base16-ocean.dark")
   - Detect file language from `diff.file_path` once per diff
   - In `format_diff_line()`:
     - If syntax highlighter available and language detected, get highlighted spans for content
     - Apply syntax colors as foreground colors
     - Keep diff semantic colors (additions/deletions) for background/gutter
   - Keep existing gutter, line numbers, and inline diff logic intact

4. **Color blending strategy**
   - **For additions (+)**: Apply syntax foreground colors, keep green diff coloring for gutter
   - **For deletions (-)**: Apply syntax foreground colors, keep red diff coloring for gutter
   - **For context ( )**: Apply syntax colors directly to foreground
   - **For inline diffs**: This is tricky - inline diff uses background colors to highlight word changes
     - Strategy: Skip syntax highlighting on lines with inline diffs, OR
     - Apply syntax to foreground, inline diff to background (test both approaches)

5. **Test with various file types**
   - Test with Rust files (.rs)
   - Test with markdown (.md)
   - Test with JSON/YAML/TOML config files
   - Test with unknown extensions (should fallback gracefully)
   - Verify line numbers and gutter still work
   - Verify inline diffs still highlight word changes

### Success Criteria for Phase 1
- Syntax highlighting visible in diffs
- No performance degradation for small/medium diffs (<1000 lines)
- Existing features (inline diff, line numbers, gutter) still work
- Graceful fallback for unsupported languages

## Phase 2: Performance Optimization (Future)

### Goals
- Handle large diffs without blocking the UI
- Cache highlighted content to avoid re-highlighting on scroll
- Optional: Make highlighting toggleable via keybind

### Potential Approaches

1. **Caching Strategy**
   - Add cache field to `DiffView`: `HashMap<(usize, usize), Vec<Span>>`
   - Key: (hunk_index, line_index), Value: pre-rendered spans
   - Populate cache lazily as lines are rendered
   - Clear cache on theme change or diff reload

2. **Background Processing**
   - Use `tokio` or similar to highlight lines off the main thread
   - Send highlighted spans back via channel
   - Update cache when results arrive
   - Show unhighlighted content immediately, update when ready

3. **Toggle Feature**
   - Add config option: `enable_syntax_highlighting: bool`
   - Add keybind (e.g., 'h') to toggle at runtime
   - Store state in `DiffView`

4. **Lazy Loading**
   - Only highlight visible lines + small buffer
   - Highlight additional lines as user scrolls
   - Combined with caching for best performance

### Implementation Order for Phase 2
1. Add basic caching (quick win)
2. Add toggle keybind (user control)
3. Implement background highlighting (if needed based on performance testing)
4. Add lazy viewport-based highlighting (if needed)

## Technical Decisions

### Which syntect theme to use?
- Start with "base16-ocean.dark" (widely compatible with dark terminals)
- Could test: "Solarized (dark)", "Monokai", "base16-eighties.dark"
- Future: Add config option to match mahgit's theme selection

### Where to initialize SyntaxSet?
- Initialize once in `DiffView::new()` or lazily on first use
- SyntaxSet is heavy (~MB of data), but initialization is one-time cost
- Could later lift to App level and share across views (optimization)

### How to handle language detection?
- Use `SyntaxSet::find_syntax_for_file()` with full path
- Fallback order: extension → first line (shebang) → none
- No highlighting is better than wrong highlighting

### How to blend colors?
- Diff semantic colors (green/red for +/-) are more important than syntax colors
- Strategy: Keep diff colors for line type indication (gutter, background)
- Apply syntax highlighting to foreground text only
- For inline diffs: Keep change backgrounds (word highlights), syntax on foreground

### Interaction with inline diffs?
- Inline diffs highlight word-level changes with background colors
- Syntax highlighting uses foreground colors
- These should be compatible: syntax on foreground, inline diff on background
- Need to test this carefully

## File Structure Changes

```
src/
├── diff/
│   ├── mod.rs           (export syntax module)
│   ├── syntax.rs        (NEW: SyntaxHighlighter)
│   ├── generator.rs
│   ├── inline.rs
│   ├── parser.rs
│   └── navigator.rs
├── ui/
│   └── diff_view.rs     (integrate syntax highlighting)
└── theme.rs             (might add syntect theme mappings later)
```

## Risk Mitigation

1. **Performance Risk**: Phase 1 might be slow for large files
   - Mitigation: Test with large files, document performance, plan Phase 2 if needed

2. **Color Conflict Risk**: Syntax colors might clash with diff colors or inline diffs
   - Mitigation: Careful color blending strategy, extensive testing, make toggleable

3. **Dependency Risk**: syntect is large (~1-2MB with default features)
   - Mitigation: Acceptable for the functionality, widely used (bat, delta, etc.)

4. **Maintenance Risk**: syntect theme might not match mahgit theme
   - Mitigation: Start simple, iterate based on user feedback

## Open Questions

1. **Should syntax highlighting be on by default or opt-in?**
   - Recommendation: On by default with toggle option (Phase 2)
   - Test with users first

2. **Should we highlight only the visible viewport or all lines?**
   - Phase 1: All visible lines (simple)
   - Phase 2: Viewport-based + caching (optimized)

3. **Which syntect theme provides best contrast with diff colors?**
   - Test multiple themes: "base16-ocean.dark", "Solarized (dark)", "base16-eighties.dark"
   - Pick one that works well with both gruvbox-dark and github-dark mahgit themes

4. **How to handle inline diffs + syntax highlighting interaction?**
   - Option A: Skip syntax highlighting on lines with inline diffs
   - Option B: Apply both (syntax on fg, inline on bg)
   - Need to test both approaches

5. **Should we support custom syntect themes?**
   - Not in Phase 1
   - Phase 2: Consider adding to config file

## Testing Plan

### Manual Testing
1. Create test files in `.tmp/`:
   - Rust file with syntax errors and various constructs
   - Markdown with code blocks
   - JSON/YAML config files
   - Unknown file extension
2. Make changes and view diffs
3. Verify syntax highlighting appears
4. Verify inline diffs still work
5. Test with different mahgit themes
6. Test scrolling performance on large files (1000+ lines)

### Unit Tests
- Test language detection logic
- Test syntect to ratatui color conversion
- Test fallback when language not detected
- Test that format_diff_line still produces correct spans

## Implementation Dependencies

### Cargo.toml Changes
```toml
[dependencies]
syntect = "5.2"  # or latest version
```

### Key syntect APIs to Use
- `SyntaxSet::load_defaults_newlines()` - Load syntax definitions
- `ThemeSet::load_defaults()` - Load themes
- `SyntaxSet::find_syntax_for_file()` - Detect language
- `HighlightLines::highlight_line()` - Highlight a single line
- `Style::foreground` - Get foreground color from syntect

## Success Metrics

### Phase 1
- [ ] Code is syntax highlighted in diffs
- [ ] No visual regression in existing features
- [ ] Performance acceptable for files <1000 lines
- [ ] Graceful degradation for unsupported file types
- [ ] No crashes or errors

### Phase 2 (Future)
- [ ] Large files (5000+ lines) scroll smoothly
- [ ] Cache hit rate >80% during normal scrolling
- [ ] Toggle feature works as expected
- [ ] Memory usage reasonable (<10MB for cache)
