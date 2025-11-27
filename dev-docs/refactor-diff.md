# Unified Diff Rendering Component

## Goal
Create a single, reusable diff rendering implementation that can be used in multiple contexts (inline in status view, full-screen diff view, future log view).

## Current State Analysis

### Code Duplication Identified

**Two separate implementations exist:**

1. **`DiffView`** (`src/ui/diff_view.rs`) - Standalone, stateful component:
   - Full diff rendering with borders, title, scrollbar
   - Viewport and scroll management (scroll position, viewport height)
   - Hunk navigation (current_hunk_index, navigate_to_next/previous_hunk)
   - Sticky header support when scrolled past hunk headers
   - Error handling (binary files, diff errors)
   - ~1200 lines including tests

2. **Inline diff in `StatusView`** (`src/ui/status_view.rs:478-631`) - Embedded rendering:
   - Renders diffs as ListItems within status view
   - Similar line formatting logic
   - Tightly coupled to list rendering context
   - ~150 lines of diff-specific code

**Duplicated code patterns:**

- `calculate_line_number_widths()` - **Identical** (diff_view.rs:337-370, status_view.rs:675-703)
- `format_line_number()` - **Identical** (diff_view.rs:367-370, status_view.rs:705-708)
- `digit_width()` - **Identical** helper function
- `apply_hunk_highlight()` - **Similar** logic (diff_view.rs:325-335, status_view.rs:667-673)
- Line formatting - **Similar** but different contexts:
  - DiffView: Generates `Vec<Line>` for Paragraph widget
  - StatusView: Generates `Vec<ListItem>` with indentation for inline display

**Existing helper:**
- `InlineDiffRenderer` (`src/ui/inline_diff.rs`) - Already handles word-level diff highlighting within lines

## Proposed Architecture

### New Component: `DiffRenderer`

Create a **stateless, reusable rendering component** in `src/ui/diff_renderer.rs`:

```rust
pub struct DiffRenderer<'a> {
    config: &'a Config,
}

impl<'a> DiffRenderer<'a> {
    pub fn new(config: &'a Config) -> Self;

    // Core line formatting (stateless)
    pub fn format_diff_line(
        &self,
        diff_line: &DiffLine,
        line_number_widths: Option<&LineNumberWidths>,
        is_active_hunk: bool,
        prefix: Option<&str>,  // For indentation in inline mode
    ) -> Line<'static>;

    // Generate all diff lines (stateless)
    pub fn generate_diff_lines(
        &self,
        diff: &Diff,
        active_hunk_index: Option<usize>,
        prefix: Option<&str>,  // For indentation in inline mode
    ) -> Vec<Line<'static>>;

    // Utility methods
    pub fn calculate_line_number_widths(diff: &Diff) -> LineNumberWidths;
    pub fn format_line_number(number: Option<usize>, width: usize) -> String;
}

#[derive(Debug, Clone, Copy)]
pub struct LineNumberWidths {
    pub old: usize,
    pub new: usize,
}
```

### Refactored Components

**1. `DiffView` (refactored)**
- Remove duplicated formatting logic
- Use `DiffRenderer` for all line generation
- Keep viewport/scroll state management
- Keep navigation state (current_hunk_index)
- Keep sticky header rendering logic
- Keep error/binary file handling

**2. `StatusView` (refactored)**
- Remove duplicated formatting logic
- Use `DiffRenderer` for inline diff rendering
- Wrap generated `Line`s in `ListItem`s for inline display
- Keep sticky overlay logic (different from DiffView's sticky header)

**3. Future `LogView` (not yet implemented)**
- Will use `DiffRenderer` for commit diff display
- Can choose full-screen or inline rendering style

## Key Design Decisions

### 1. Stateless Renderer
The `DiffRenderer` will be **stateless** - it takes data and config, returns formatted lines. All state (scroll position, current hunk, expanded hunks) stays in the view components.

**Rationale:** Maximizes reusability - different views have different state needs.

### 2. Return Type: `Vec<Line<'static>>`
The renderer returns ratatui `Line`s, not a specific widget type.

**Rationale:**
- `DiffView` wraps lines in `Paragraph` widget
- `StatusView` wraps lines in `ListItem`s
- Most flexible return type

### 3. Indentation via Prefix
For inline mode, caller passes a prefix string (e.g., "      " for 6-space indent).

**Rationale:**
- Keeps renderer agnostic to context
- StatusView controls its own indentation scheme

### 4. Shared Helper: `InlineDiffRenderer`
Continue using existing `InlineDiffRenderer` for word-level highlighting.

**Rationale:** Already well-tested and working.

### 5. Theme and Config
Pass `Config` reference containing theme to the renderer.

**Rationale:** All styling decisions live in theme config.

## Implementation Steps

### Step 1: Create `DiffRenderer` module
- Create `src/ui/diff_renderer.rs`
- Define `DiffRenderer` struct and `LineNumberWidths`
- Copy and adapt line number utility functions
- Add module to `src/ui/mod.rs`

### Step 2: Implement core rendering methods
- Implement `calculate_line_number_widths()` (utility fn)
- Implement `format_line_number()` (utility fn)
- Implement `format_diff_line()` - core line formatter
  - Handle line type colors (addition/deletion/context)
  - Handle gutter styling
  - Handle line numbers (optional)
  - Handle hunk highlighting
  - Handle inline word-level diffs via `InlineDiffRenderer`
  - Support optional prefix for indentation

### Step 3: Implement diff generation
- Implement `generate_diff_lines()`
  - Iterate through hunks
  - Format hunk headers
  - Format each diff line in hunk
  - Apply active hunk highlighting based on `active_hunk_index`

### Step 4: Refactor `DiffView`
- Replace `generate_diff_lines()` with call to `DiffRenderer::generate_diff_lines()`
- Replace `format_diff_line()` with call to `DiffRenderer::format_diff_line()`
- Replace `calculate_line_number_widths()` with call to `DiffRenderer::calculate_line_number_widths()`
- Replace `format_line_number()` with call to `DiffRenderer::format_line_number()`
- Remove `apply_hunk_highlight()` (now in DiffRenderer)
- Remove `LineNumberWidths` type (now in DiffRenderer)
- Keep all state management (scroll_position, current_hunk_index, viewport_height)
- Keep sticky header rendering
- Keep error/binary handling

### Step 5: Refactor `StatusView`
- Remove `calculate_line_number_widths()` implementation
- Remove `format_line_number()` implementation
- Remove `apply_hunk_highlight()` implementation
- Remove `LineNumberWidths` type
- Replace inline diff formatting in `add_inline_diff_items_with_selection()`:
  - Use `DiffRenderer` to generate diff lines
  - Wrap each `Line` in a `ListItem` with appropriate indentation prefix
  - Preserve hunk collapse/expand logic (StatusView-specific)
  - Preserve hunk selection highlighting

### Step 6: Update imports and exports
- Export `DiffRenderer` and `LineNumberWidths` from `src/ui/mod.rs`
- Update imports in `diff_view.rs`
- Update imports in `status_view.rs`
- Move `LineNumberWidths` from local definitions to shared type

### Step 7: Testing
- Run existing `DiffView` tests - should pass unchanged
- Run existing `StatusView` integration tests
- Add tests for `DiffRenderer` itself:
  - Test line number width calculation
  - Test line formatting with/without line numbers
  - Test hunk highlighting
  - Test prefix/indentation support
  - Test inline word-level diff integration

### Step 8: Code cleanup
- Remove dead code (if any)
- Run `cargo fmt`
- Run `cargo clippy`
- Address any warnings

## Expected Outcomes

1. **Single source of truth** for diff line formatting
2. **~300 lines of duplicate code eliminated**
3. **Easier to maintain** - changes to diff rendering happen in one place
4. **Future-ready** - log view can reuse the same renderer
5. **No behavior changes** - existing functionality preserved
6. **Test coverage maintained** - all existing tests continue to pass

## Trade-offs and Risks

### Trade-offs
- **Additional abstraction layer**: One more component to understand
- **Slight performance overhead**: Function call indirection (negligible)
- **Initial refactoring effort**: Moderate - touches two core UI components

### Risks and Mitigations
- **Risk:** Breaking existing functionality during refactor
  - **Mitigation:** Step-by-step approach, run tests after each step

- **Risk:** Introducing subtle rendering differences
  - **Mitigation:** Careful comparison of old vs new implementation, visual testing

- **Risk:** Making renderer too generic/complex
  - **Mitigation:** Start simple, only add flexibility as needed

## Open Questions

None - the design is straightforward given existing patterns.

## Files to Create
- `src/ui/diff_renderer.rs` (new)

## Files to Modify
- `src/ui/mod.rs` (add diff_renderer module)
- `src/ui/diff_view.rs` (refactor to use DiffRenderer)
- `src/ui/status_view.rs` (refactor to use DiffRenderer)

## Success Criteria
- [ ] All existing tests pass
- [ ] `cargo clippy` reports no warnings in modified files
- [ ] Visual inspection shows no rendering changes in status view
- [ ] Visual inspection shows no rendering changes in diff view
- [ ] Code duplication reduced by ~300 lines
- [ ] DiffRenderer has its own test coverage
