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
}
```

### 2. Input Handling Changes

#### New Commands
```rust
pub enum Command {
    // Existing commands...
    
    // New accordion commands
    ToggleAccordion,        // Tab key - contextually toggle section or file diff
    ScrollInlineDiffUp,     // j/k when in expanded diff
    ScrollInlineDiffDown,
    NextHunkInline,         // n - next hunk in inline diff
    PrevHunkInline,         // p - previous hunk in inline diff
    StageHunkInline,        // s - stage current hunk in inline diff
    UnstageHunkInline,      // u - unstage current hunk in inline diff
}
```

#### Key Bindings
- `Tab` → Contextually toggle section or file diff (context-aware)
  - When cursor is on section header: toggle section collapsed/expanded
  - When cursor is on file: toggle file diff inline display
- `j/k` → Navigate files or scroll diff content (context-aware)
- `n/p` → Navigate hunks when diff is expanded
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
    pub fn move_down(&mut self) {
        if self.is_diff_expanded_for_current_file() {
            // Navigate within diff content
            self.scroll_current_diff_down();
        } else {
            // Navigate to next file/section
            self.move_to_next_file();
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

#### Phase 2: Inline Diff Display
- Remove full-screen diff view mode
- Implement inline diff rendering in StatusView
- Extend Tab key handling for file diff toggle (context-aware)
- Implement diff caching system

**Note**: After Phase 1 implementation, the key binding strategy was refined:
- Tab = Section collapse/expand (simple, predictable)
- Enter = Diff view (standard Git UI convention)
- This provides clearer UX than the original context-aware Tab proposal

#### Phase 3: Enhanced Navigation
- Add context-aware navigation (j/k behavior)
- Implement inline hunk navigation (n/p keys)
- Add inline hunk staging (s/u keys)
- Refine Tab key context detection logic
- Optimize scrolling behavior for mixed content

#### Phase 4: Polish and Optimization
- Add visual indicators for expanded states
- Implement smooth scrolling behavior
- Add keyboard shortcuts help updates (emphasize Tab key context behavior)
- Performance tuning for large diffs

## Benefits

### User Experience
- **Contextual Awareness**: Always see the file list and current selection
- **Quick Comparison**: Toggle multiple file diffs without losing context
- **Efficient Navigation**: Reduced cognitive load from view switching
- **Magit-like Workflow**: Mirrors the accordion behavior of Magit

### Technical Benefits
- **Simplified State Management**: Single view mode reduces complexity
- **Better Responsiveness**: No full-screen transitions
- **Incremental Loading**: Generate diffs only when needed
- **Memory Efficiency**: Cache management for better performance

## Implementation Files

### Primary Changes
- `src/ui/mod.rs` - Remove Diff view mode, add accordion state
- `src/ui/status_view.rs` - Major rewrite for inline diff rendering
- `src/ui/navigation.rs` - Add section/diff state management
- `src/ui/input.rs` - Add new command mappings

### New Files
- `src/ui/accordion.rs` - Accordion-specific rendering logic
- `src/ui/inline_diff.rs` - Inline diff rendering utilities

### Configuration
- Add user preferences for default section collapsed states
- Add keybinding customization support
- Add diff display options (context lines, syntax highlighting)

## Testing Strategy

### Unit Tests
- Section collapse/expand logic
- Inline diff generation and caching
- Navigation state transitions
- Key mapping correctness

### Integration Tests  
- Full workflow testing (navigate, expand, diff, stage)
- Performance testing with large repositories
- Memory usage validation
- Cross-platform keyboard handling

### User Testing
- A/B testing between accordion and full-screen modes
- Workflow efficiency measurements
- Accessibility compliance testing

This design provides a comprehensive roadmap for implementing an accordion-style UI that maintains context while providing efficient Git operations, closely matching the Magit user experience.

## Implementation Status

### ✅ Phase 1 Complete (January 2025)

**What Works Now:**
- Section collapse/expand with Tab key and visual indicators (▶/▼)
- Enter key opens diff view for selected files
- All existing diff operations preserved (j/k navigation, n/p hunk nav, S/U staging)
- Clean separation between section management and diff viewing

**Files Modified:**
- `src/ui/navigation.rs` - Added `SectionCollapsedState` and collapse methods
- `src/ui/input.rs` - Added `ToggleAccordion` command and Enter key mapping
- `src/ui/mod.rs` - Added `toggle_accordion()` method  
- `src/ui/status_view.rs` - Updated rendering to respect collapse states

**Key Insights from Implementation:**
1. **Simplified Key Bindings**: Tab for sections, Enter for diffs is more intuitive than complex contextual behavior
2. **Visual Clarity**: Collapse indicators (▶/▼) provide immediate visual feedback
3. **Backward Compatibility**: All existing functionality preserved while adding new capabilities
4. **Foundation Ready**: Clean architecture ready for Phase 2 inline diff implementation

**Ready for Phase 2**: The section collapse foundation is solid and ready for inline diff rendering implementation.