# Phase 4: Diff Viewer Foundation - Implementation Plan

**Goal**: Display file differences in a readable, navigable format

## Scope

**In Scope**:
- Generate diffs for unstaged, staged, and HEAD comparisons
- Display diffs with scrolling and navigation
- Switch between status and diff views
- Handle binary files and large files gracefully

**Out of Scope** (Phase 5):
- Interactive hunk staging
- Line-level operations
- Merge conflict resolution

## Architecture

### 1. Diff Engine (`src/diff/`)

```rust
#[derive(Debug, Clone)]
pub struct Diff {
    pub file_path: String,
    pub context: DiffContext,
    pub hunks: Vec<DiffHunk>,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffContext {
    WorkingTreeToIndex,    // Unstaged changes
    IndexToHead,           // Staged changes
    WorkingTreeToHead,     // All changes
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub content: String,
    pub old_line_number: Option<u32>,
    pub new_line_number: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLineType {
    Context,
    Addition,
    Deletion,
    NoNewlineWarning,
}
```

### 2. Enhanced View System

```rust
#[derive(Clone, PartialEq)]
pub enum ViewType {
    Status,
    Diff(DiffViewState),
}

#[derive(Clone, PartialEq)]
pub struct DiffViewState {
    pub file_path: String,
    pub diff_context: DiffContext,
    pub scroll_position: usize,
    pub hunk_index: Option<usize>,
}
```

### 3. Diff View Component (`src/ui/diff_view.rs`)

```rust
pub struct DiffView {
    diff: Diff,
    scroll_position: usize,
    viewport_height: usize,
}

impl DiffView {
    pub fn new(diff: Diff) -> Self;
    pub fn render(&self, frame: &mut Frame, area: Rect);
    pub fn scroll_up(&mut self, lines: usize);
    pub fn scroll_down(&mut self, lines: usize);
    pub fn jump_to_next_hunk(&mut self);
    pub fn jump_to_previous_hunk(&mut self);
}
```

### 4. Input System Extensions

```rust
pub enum Command {
    // Existing commands...
    
    EnterDiffView,
    ExitDiffView,
    ScrollDiffUp,
    ScrollDiffDown,
    PageDiffUp,
    PageDiffDown,
    JumpToNextHunk,
    JumpToPreviousHunk,
    GoToTopOfDiff,
    GoToBottomOfDiff,
}
```

**Key Bindings**:
- `Enter` - Enter diff view from status buffer
- `q`/`Escape` - Exit diff view
- `j`/`k`/`↓`/`↑` - Line scrolling
- `f`/`b`/`Page Down`/`Page Up` - Page scrolling
- `n`/`p` - Next/previous hunk
- `gg`/`G` - Top/bottom of diff

## Implementation Steps

### Step 1: Diff Generation ✅ COMPLETED
- ✅ Create `src/diff/mod.rs` and `src/diff/generator.rs`
- ✅ Implement `DiffGenerator` using git2
- ✅ Handle working tree, index, and HEAD comparisons
- ✅ Add error handling for edge cases

### Step 2: Diff Parsing ✅ COMPLETED
- ✅ Create `src/diff/parser.rs`
- ✅ Convert git2 diffs to structured format
- ✅ Parse hunks and lines with proper numbering
- ✅ Handle binary files and special cases

### Step 3: Diff View Component ✅ COMPLETED
- ✅ Create `src/ui/diff_view.rs`
- ✅ Implement diff rendering with colors
- ✅ Add scrolling mechanics and viewport calculation
- ✅ Implement hunk navigation

### Step 4: View Integration
- Extend `App` struct with diff view state
- Add view switching logic
- Implement Enter key handling from status buffer
- Add context-aware diff generation

### Step 5: Input Handling
- Add new commands to input system
- Implement diff navigation key bindings
- Update help text
- Add command routing for diff view

### Step 6: Error Handling
- Handle binary files gracefully
- Implement size limits for large files
- Add meaningful error messages
- Test terminal compatibility

## Testing

### Unit Tests
- Diff generation with various file states
- Diff parsing correctness
- View scrolling and navigation logic
- Error handling edge cases

### Integration Tests
- Status to diff workflow
- Context switching between file states
- Binary file and large file handling
- Terminal resize handling

## Success Criteria

**Functional**:
- View diffs for any file from status buffer
- Navigate through diff content smoothly
- Switch between unstaged/staged contexts
- Handle all file types appropriately

**Non-Functional**:
- Responsive scrolling and navigation
- Reasonable memory usage
- Graceful error handling
- Consistent visual design

## Architecture Notes

**Ready for Phase 5**:
- Diff structure supports hunk operations
- Command system extensible for hunk staging
- Error handling supports partial operations
- UI framework supports fine-grained interaction

**Integration Points**:
- Enhanced use of git2 diff methods
- Extended ViewType enum in UI module
- New commands in input system
- Context-aware operation detection