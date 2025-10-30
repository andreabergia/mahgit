# Accordion-Style UI Design Plan

## Overview

Transform the current full-screen diff view into an inline accordion-style interface where:
- File diffs appear inline beneath the file name in the main view
- Sections (Staged, Unstaged, Untracked, Conflicted) become collapsible/expandable
- Tab key toggles both section visibility and file diff visibility contextually

## Current Architecture Analysis

### Current UI Structure
- `App` holds the main state with `ViewType` enum: `Status` or `Diff(DiffViewState)`
- `StatusView` renders the file list with sections
- `DiffView` renders full-screen diff content
- Navigation handled by `NavigationState` with section and file selection
- Diff generation handled by `DiffGenerator`

### Current Behavior Issues
- Diff view takes over entire screen, losing context of file list
- No way to quickly compare multiple files
- Context switching requires exiting diff view completely

## New Architecture Design

### 1. UI State Management

#### Modified ViewType
```rust
#[derive(Clone, PartialEq)]
pub enum ViewType {
    Status, // Remove Diff variant - everything stays in Status view
}

#[derive(Clone, PartialEq)]
pub struct SectionCollapsedState {
    pub conflicted: bool,
    pub unstaged: bool, 
    pub untracked: bool,
    pub staged: bool,
}

pub struct InlineDiffState {
    pub file_path: String,
    pub diff_context: crate::diff::DiffContext,
    pub diff: Option<Diff>,
    pub expanded: bool,
}
```

#### Enhanced NavigationState
```rust
pub struct NavigationState {
    // Existing fields
    current_section: StatusSection,
    selected_index: usize,
    sections: Vec<SectionInfo>,
    
    // New fields for accordion behavior
    section_collapsed: SectionCollapsedState,
    file_diffs: HashMap<String, InlineDiffState>,
    current_diff_scroll: usize,
    focus: NavigationFocus,
}

pub enum NavigationFocus {
    File,
    Diff,
}
```

### 2. Input Handling Changes

#### New Commands
```rust
pub enum Command {
    // Existing commands...
    
    // New accordion commands
    ToggleAccordion,        // Tab key - contextually toggle section or file diff
    ScrollInlineDiffUp,     // k when in expanded diff
    ScrollInlineDiffDown,   // j when in expanded diff
    NextHunkInline,         // n - jump to next hunk/file header from anywhere in the list
    PrevHunkInline,         // p - jump to previous hunk/file header from anywhere in the list
    PageForward,            // space - page scroll within the active focus (diff or list)
    StageHunkInline,        // s - stage current hunk in inline diff
    UnstageHunkInline,      // u - unstage current hunk in inline diff
}
```

#### Key Bindings
- `Tab` → Contextually toggle section or file diff (context-aware)
  - When cursor is on section header: toggle section collapsed/expanded
  - When cursor is on file: toggle file diff inline display
- `Arrow Up/Down` / `j/k` → Scroll inline diff content without moving the selection; when no diff is focused they fall back to moving between visible rows.
- `n/p` → Jump directly to the next/previous hunk header or file row regardless of current focus, skipping over inline diff body lines for fast traversal.
- `Space` → Page-scroll the active focus: when a diff is focused, scroll its viewport by one screenful; otherwise page the status list while keeping the current selection in view.
- `s/u` → Stage/unstage hunk when diff is expanded

### 3. Enhanced StatusView

#### Section Rendering with Collapse States
```rust
impl StatusView {
    fn render_section_header(&self, section: StatusSection, collapsed: bool) -> ListItem {
        let icon = if collapsed { "▶" } else { "▼" };
        let style = if self.navigation.current_section() == section {
            Style::default().bg(Color::DarkGray).fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Cyan)
        };
        
        ListItem::new(format!("{} {} ({})", icon, section_name, file_count))
            .style(style)
    }
    
    fn render_file_with_inline_diff(&self, file: &FileEntry, expanded_diff: Option<&Diff>) -> Vec<ListItem> {
        let mut items = vec![];
        
        // File name line
        items.push(self.create_file_item(file));
        
        // Inline diff content if expanded
        if let Some(diff) = expanded_diff {
            items.extend(self.create_inline_diff_items(diff));
        }
        
        items
    }
}
```

### 4. Inline Diff Rendering

#### Compact Diff Display
```rust
impl StatusView {
    fn create_inline_diff_items(&self, diff: &Diff) -> Vec<ListItem> {
        let mut items = vec![];
        
        for hunk in &diff.hunks {
            // Hunk header with indentation
            items.push(ListItem::new(format!("    {}", hunk.header.raw))
                .style(Style::default().fg(Color::Cyan)));
            
            // Diff lines with deeper indentation
            for line in &hunk.lines {
                let (prefix, color) = match line.line_type {
                    LineType::Addition => ("+", Color::Green),
                    LineType::Deletion => ("-", Color::Red),
                    LineType::Context => (" ", Color::White),
                    LineType::NoNewlineEOF => ("\\", Color::Yellow),
                };
                
                items.push(ListItem::new(format!("      {}{}", prefix, line.content))
                    .style(Style::default().fg(color)));
            }
            
            // Add spacing between hunks
            items.push(ListItem::new(""));
        }
        
        items
    }
}
```

### 5. Navigation Logic Changes

#### Context-Aware Navigation
```rust
impl NavigationState {
    pub fn scroll_down(&mut self) {
        // Both arrow keys and j feed into this path.
        if self.focus == NavigationFocus::Diff && self.advance_inline_hunk(1) {
            return;
        }

        self.focus = NavigationFocus::File;
        self.move_to_next_row();
        self.snap_viewport_to_selection();
    }

    pub fn scroll_up(&mut self) {
        // Shared path for ↑/k as well.
        if self.focus == NavigationFocus::Diff && self.advance_inline_hunk(-1) {
            return;
        }

        self.focus = NavigationFocus::File;
        self.move_to_previous_row();
        self.snap_viewport_to_selection();
    }

    pub fn jump_to_next_change(&mut self) {
        // Shared helper for `n` and the status-pane equivalents (next section, next file, next hunk header).
        let target = self.index_of_next_marker(self.selected_index);
        self.set_selection(target);
        self.snap_viewport_to_selection();
    }

    pub fn page_forward(&mut self) {
        match self.focus {
            NavigationFocus::Diff => self.page_diff_down(),
            NavigationFocus::File => self.page_status_list_down(),
        }
    }
    
    pub fn toggle_accordion(&mut self, repository: &Repository) {
        // Context-aware toggle behavior based on current selection
        if self.is_on_section_header() {
            // Toggle section collapsed/expanded
            let current_section = self.current_section;
            match current_section {
                StatusSection::Conflicted => self.section_collapsed.conflicted = !self.section_collapsed.conflicted,
                StatusSection::Unstaged => self.section_collapsed.unstaged = !self.section_collapsed.unstaged,
                StatusSection::Untracked => self.section_collapsed.untracked = !self.section_collapsed.untracked,
                StatusSection::Staged => self.section_collapsed.staged = !self.section_collapsed.staged,
            }
        } else if let Some(selected_file) = self.get_selected_file(status) {
            // Toggle file diff display
            let file_key = selected_file.path.clone();
            
            match self.file_diffs.get_mut(&file_key) {
                Some(diff_state) => {
                    // Toggle existing diff
                    diff_state.expanded = !diff_state.expanded;
                }
                None => {
                    // Generate and expand new diff
                    self.generate_and_store_diff(&selected_file, repository);
                }
            }
        }
    }
    
    fn is_on_section_header(&self) -> bool {
        // Logic to determine if cursor is on a section header vs a file
        // This would check if selected_index == 0 in each section or similar logic
        self.selected_index == 0 && self.is_at_section_boundary()
    }
}
```

### 6. Performance Optimizations

#### Lazy Diff Generation
- Only generate diffs when explicitly requested (Enter key)
- Cache generated diffs until repository state changes
- Implement diff invalidation on file modifications

#### Memory Management
```rust
impl NavigationState {
    pub fn invalidate_diff_cache(&mut self) {
        self.file_diffs.clear();
    }
    
    pub fn invalidate_file_diff(&mut self, file_path: &str) {
        self.file_diffs.remove(file_path);
    }
}
```

### 7. Migration Strategy

#### Phase 1: Section Collapsing ✅ COMPLETED
- ✅ Implement section collapsed state tracking
  - Added `SectionCollapsedState` struct with per-section collapse state
  - Enhanced `NavigationState` with `is_section_collapsed()` and `toggle_section_collapsed()` methods
- ✅ Add contextual Tab key handling for section toggle
  - Created new `ToggleAccordion` command for context-aware behavior
  - Tab key now toggles section collapsed/expanded state in Status view
  - Preserved existing Tab behavior in Diff view (exits to Status)
- ✅ Modify StatusView to respect collapsed states
  - Added collapse indicators (▶/▼) to section headers
  - Files hidden when section is collapsed
  - Updated both file rendering methods to respect collapse state
- ✅ Keep existing diff view functionality
  - Added Enter key mapping for intuitive diff access (`EnterDiffView`)
  - All existing diff navigation and hunk operations preserved
  - Updated help text to reflect new key bindings: Tab (section toggle) + Enter (diff view)

#### Phase 2: Inline Diff Display ⚡ IN PROGRESS
- ✅ Remove full-screen diff view mode (ViewType::Diff variant eliminated)
- ✅ Add inline diff state management to NavigationState (InlineDiffState, file_diffs HashMap)
- ✅ Implement basic inline diff toggle functionality (Enter key now toggles inline diffs)
- ✅ Implement inline diff rendering in StatusView
  - Added `add_inline_diff_items()` method for rendering diff content inline
  - Supports binary file detection with appropriate placeholder text
  - Renders hunk headers and diff lines with proper indentation and coloring
  - Shows loading placeholder when diff is being generated
  - Applied to both FileEntry and string-based file sections
- ✅ Extend Tab key handling for file diff toggle (context-aware)
  - Added `is_on_section_header()` method to NavigationState for context detection
  - Modified `toggle_accordion()` to use context-aware logic: section toggle OR file diff toggle
  - Infrastructure in place for future enhancement of section header navigation
  - Currently defaults to file diff toggle behavior (preserves existing UX)
- ✅ Implement diff caching system
  - Added `NavigationState::has_cached_diff_for()` to detect reusable inline diffs tied to their git context
  - `App::toggle_inline_diff()` now reuses cached diffs when collapsing/re-expanding files instead of regenerating
  - Manual refresh clears the cache to avoid stale data and a unit test covers the new helper

**Note**: After Phase 1 implementation, the key binding strategy was refined:
- Tab = Section collapse/expand (simple, predictable)
- Enter = Diff view (standard Git UI convention)
- This provides clearer UX than the original context-aware Tab proposal

#### Phase 3: Enhanced Navigation
The next batch of work should land in the order below because each step sets up state or UX expectations that the later ones rely on.

1. **Split selection movement from viewport scrolling** ✅
   - ✅ Routed both arrow keys and `j/k` through the same diff-first scroll path so inline content absorbs navigation before the list moves.
   - ✅ Left the mouse wheel mapped to viewport-only scrolling to preserve manual scroll control.
   - ✅ Wired `n/p` into the jump-to-change helpers so users can skip diff bodies quickly.
   - ✅ Added a context-aware `PageForward` command behind `Space` so paging works in both the list and inline diff focus.
   - ✅ Stored viewport metrics plus the manual scroll flag in `NavigationState` and routed them through `StatusView` to avoid reliance on `ListState`’s implicit scrolling.
   - ⏳ Update help text and documentation so users understand the arrow, vim, n/p, and space distinctions.

2. **Add an explicit inline diff focus state** ✅
   - ✅ When Tab expands a file diff, leave focus on the file row; only enter “hunk focus” when the user issues a movement command that targets the diff (e.g., `j/k` traversal).
   - ✅ Track this focus in navigation state (e.g., `NavigationFocus::File | NavigationFocus::Diff`) so other commands can tell whether a hunk is actually selected.
   - ✅ Ensure collapsing a diff or switching files resets the focus back to the file to avoid stale hunk selections.

3. **Unify staging shortcuts around `s`/`u`** ✅
   - ✅ Route lowercase `s/u` through a context-aware staging helper that stages/unstages hunks when an inline diff is active, otherwise acts on the whole file.
   - ✅ Remove the Shift+`S`/`U` bindings and the corresponding command variants once the new logic is in place.
   - ✅ Refresh the help overlay and any inline documentation to reflect the simplified key set.

#### Space Key Follow-Up: Paging Behavior
- Bind `<space>` to the shared paging helper so expanded inline diffs scroll by one viewport while the status list paginates when no diff is focused.
- Consider adding `Shift+<space>` as a future enhancement for reverse paging once the forward behavior ships and feels solid.

4. **Fix redraw artifacts in wide terminals** ✅
   - ✅ Audit `StatusView` rendering and ensure every expanded/collapsed path writes full-width blank lines (or uses `Clear`) so no stale text remains.
   - ✅ Exercise the UI with very wide terminals and nested expand/collapse cycles to confirm there are no lingering ghost lines.

#### Phase 4: Polish and Optimization
- Add visual indicators for expanded states
  - Review current accordion widgets to confirm where expansion status is rendered.
  - Select arrow/chevron glyphs that match the existing TUI style and palette.
  - Update focus and toggle handlers so indicators stay in sync with state changes.
  - Validate indicator accessibility (contrast, screen-reader labels if supported).
- Implement smooth scrolling behavior
  - Profile current scroll jumps with large panels to identify jitter.
  - Introduce incremental scrolling logic that keeps redraw cost bounded.
  - Ensure keyboard repeat events and mouse wheel inputs reuse the same path.
  - Test with long diff previews in `.tmp/` repositories to check responsiveness.
- Add keyboard shortcuts help updates (emphasize Tab key context behavior)
  - Inventory current shortcut listings in the help modal/popup.
  - Add context-specific description for the `Tab` key and related navigation keys.
  - Sync documentation and on-screen hints so both reflect the new behavior.
  - Verify help overlay rendering in narrow and wide layouts.
- Performance tuning for large diffs
  - Capture baseline timings for expansion/collapse and scroll in large diffs.
  - Investigate diff rendering hotspots (e.g., line wrapping, syntax highlights).
  - Apply batching or virtualization tweaks while keeping UI responsive.
  - Re-run benchmarks to confirm improvements and document resulting metrics.
