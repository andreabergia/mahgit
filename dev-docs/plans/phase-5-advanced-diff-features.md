# Phase 5: Advanced Diff Features - Detailed Implementation Plan

## Overview
Phase 5 transforms the basic diff viewer from Phase 4 into an interactive diff manipulation interface that matches Magit's core strength: granular staging operations at the hunk level.

## Prerequisites
- Phase 4 complete: Basic diff display and navigation working
- Repository status tracking from Phase 1-3
- File-level operations from Phase 3

## Core Architecture Components

### 1. Diff Parser and Data Structures

#### Hunk Data Model
```rust
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub header: HunkHeader,
    pub lines: Vec<DiffLine>,
    pub old_range: LineRange,
    pub new_range: LineRange,
    pub stageable: bool,
    pub context_lines: usize,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub content: String,
    pub line_type: LineType,
    pub old_line_no: Option<usize>,
    pub new_line_no: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum LineType {
    Context,
    Addition,
    Deletion,
    NoNewlineEOF,
}
```

#### Diff Parsing Strategy
- **Input Sources**: Parse from git2 diff objects, not raw diff text
- **Hunk Boundaries**: Use git2's hunk callback system for accurate parsing
- **Context Preservation**: Maintain context lines for accurate patch application
- **Binary Detection**: Handle binary files gracefully (show summary, no hunk operations)

### 2. Hunk Navigation System

#### Navigation State Management
```rust
#[derive(Debug)]
pub struct HunkNavigator {
    pub current_hunk: usize,
    pub total_hunks: usize,
    pub hunk_positions: Vec<HunkPosition>, // Screen positions for rendering
    pub scroll_offset: usize,
}

#[derive(Debug)]
pub struct HunkPosition {
    pub start_line: usize,
    pub end_line: usize,
    pub screen_y: usize,
}
```

#### Key Navigation Features
- **Hunk Jumping**: `n`/`p` to jump between hunks (matching Magit)
- **Scroll Sync**: Keep current hunk visible during scroll operations
- **Visual Indicators**: Highlight current hunk with border/background
- **Hunk Preview**: Show hunk header info in status line

### 3. Interactive Staging Operations

#### Staging Workflow
1. **Hunk Selection**: Navigate to target hunk
2. **Operation Choice**: Stage/unstage current hunk
3. **Patch Generation**: Create minimal patch for selected hunk
4. **Git Operation**: Apply patch via git2 staging APIs
5. **State Refresh**: Update diff view and file status

#### Implementation Details
```rust
pub struct HunkStager {
    repo: Repository,
    current_diff: Option<DiffView>,
}

impl HunkStager {
    pub fn stage_hunk(&mut self, hunk_index: usize) -> Result<(), GitError> {
        // 1. Extract hunk as patch
        let patch = self.create_hunk_patch(hunk_index)?;
        
        // 2. Apply patch to index
        self.repo.apply_to_index(&patch)?;
        
        // 3. Refresh diff view
        self.refresh_diff_view()?;
        
        Ok(())
    }
    
    pub fn unstage_hunk(&mut self, hunk_index: usize) -> Result<(), GitError> {
        // Similar process but reverse the patch
    }
}
```

### 4. Visual Feedback System

#### Hunk Status Indicators
- **Stageable Hunks**: Green border/highlight
- **Non-stageable**: Gray/dimmed (binary files, conflicts)
- **Current Selection**: Bold border with cursor indicator
- **Operation Feedback**: Brief status messages for successful operations

#### Screen Layout Updates
```
┌─ diff_view.rs (unstaged changes) ──────────────────┐
│ @@ -45,6 +45,8 @@ impl DiffView {                        │ ← Hunk 1/3
│ -    let old_line = "removed content";              │
│ +    let new_line = "added content";                │
│ +    let another_line = "more content";             │
│      context_line();                                │
├─────────────────────────────────────────────────────┤
│ @@ -67,3 +69,5 @@ fn other_function() {                  │ ← Hunk 2/3 [CURRENT]
│ +    new_function_call();                           │
│ +    another_call();                                │
│      existing_context();                            │
└─────────────────────────────────────────────────────┘
│ s: stage hunk | u: unstage | n/p: next/prev hunk  │
```

## Implementation Phases

### Phase 5a: Hunk Parsing Foundation
**Goal**: Parse Git diffs into actionable hunk structures

**Tasks**:
1. **Diff Parser Module**:
   - Implement `DiffParser` using git2's diff iteration APIs
   - Create `DiffHunk` and `DiffLine` data structures
   - Handle edge cases: empty files, binary files, permission changes

2. **Hunk Collection**:
   - Build complete hunk list for a file diff
   - Calculate accurate line ranges for each hunk
   - Preserve context lines for patch generation

3. **Testing Framework**:
   - Create test repository with various diff scenarios
   - Unit tests for parser with edge cases
   - Integration tests with real Git repositories

**Success Criteria**:
- Parse complex diffs with multiple hunks accurately
- Handle binary files and special cases gracefully
- All hunk line numbers match git's output

### Phase 5b: Navigation Interface
**Goal**: Implement smooth navigation between hunks

**Tasks**:
1. **Navigation State**:
   - Implement `HunkNavigator` with position tracking
   - Add hunk boundary detection for keyboard navigation
   - Integrate with existing diff view scrolling

2. **Keyboard Handlers**:
   - `n`/`p` for next/previous hunk navigation
   - Ensure current hunk remains visible during navigation
   - Handle edge cases (first/last hunk boundaries)

3. **Visual Updates**:
   - Highlight current hunk with visual indicator
   - Update status line with hunk position info
   - Smooth scrolling to keep current hunk centered

**Success Criteria**:
- Navigate between hunks quickly and predictably
- Visual feedback clearly indicates current position
- Navigation works correctly with various diff sizes

### Phase 5c: Staging Operations
**Goal**: Enable staging/unstaging individual hunks

**Tasks**:
1. **Patch Generation**:
   - Create Git patches from individual hunks
   - Handle context line requirements for clean application
   - Generate reverse patches for unstaging operations

2. **Git Integration**:
   - Use git2's apply APIs for precise staging
   - Handle staging conflicts and error cases
   - Ensure atomic operations (success or rollback)

3. **State Management**:
   - Refresh diff view after operations
   - Update parent status view to reflect changes
   - Maintain navigation position after operations

**Success Criteria**:
- Stage/unstage individual hunks reliably
- Operations integrate cleanly with existing file operations
- No corruption of repository state during operations

### Phase 5d: Advanced Scenarios
**Goal**: Handle complex Git scenarios gracefully

**Tasks**:
1. **Merge Conflict Support**:
   - Parse and display conflict markers
   - Disable staging for conflicted hunks
   - Provide clear indication of conflict state

2. **Binary File Handling**:
   - Detect binary changes in diffs
   - Display summary instead of content
   - Allow file-level operations only

3. **Large Diff Optimization**:
   - Implement lazy loading for very large diffs
   - Optimize rendering performance
   - Handle memory usage for huge files

**Success Criteria**:
- Complex scenarios display appropriate information
- Performance remains acceptable with large diffs
- Error states are clearly communicated

## Testing Strategy

### Unit Tests
- **Diff Parser**: Test with generated diff scenarios
- **Hunk Operations**: Mock Git operations for isolation
- **Navigation Logic**: Test boundary conditions and state transitions

### Integration Tests
- **Real Repository Tests**: Use `.tmp/` test repositories
- **Cross-platform Testing**: Verify behavior on different OS
- **Performance Testing**: Measure with large repositories

### User Acceptance Testing
- **Magit Comparison**: Key workflows should feel familiar
- **Edge Case Handling**: Robust behavior in unusual scenarios
- **Responsiveness**: Operations should feel immediate

## Error Handling Strategy

### Git Operation Errors
- **Staging Conflicts**: Clear messages when hunks can't be applied
- **Repository State**: Handle cases where repo state changed
- **Permission Issues**: Graceful degradation for read-only scenarios

### UI Error Handling
- **Invalid Navigation**: Prevent navigation beyond bounds
- **Operation Feedback**: Immediate feedback for failed operations
- **Recovery Paths**: Allow users to retry or escape error states

## Performance Considerations

### Memory Management
- **Streaming Diffs**: Don't load entire large files into memory
- **Hunk Caching**: Cache parsed hunks for navigation performance
- **Lazy Loading**: Load hunks on-demand for huge diffs

### Responsiveness
- **Async Operations**: Use background tasks for Git operations
- **Progressive Rendering**: Show diff content as it loads
- **Interrupt Handling**: Allow cancellation of slow operations

## Integration Points

### With Phase 4 (Diff Viewer)
- Extend existing diff display with hunk boundaries
- Maintain existing navigation (scroll, search)
- Preserve performance optimizations

### With Phase 1-3 (Status Interface)
- Update file status after hunk operations
- Maintain selection state when returning to status view
- Refresh repository state consistently

### With Phase 6 (UX Polish)
- Prepare for keybinding customization
- Design for help system integration
- Consider accessibility and usability improvements

## Success Metrics

### Functional Goals
- ✅ Stage/unstage individual hunks successfully
- ✅ Navigate between hunks efficiently  
- ✅ Handle complex Git scenarios appropriately
- ✅ Maintain performance with large diffs

### User Experience Goals
- **Magit Familiarity**: Magit users feel at home
- **Discoverability**: Features are easy to learn
- **Reliability**: Operations succeed consistently
- **Performance**: Responsive even with large repositories

## Future Extensibility

### Planned Phase 6 Integration
- Custom keybindings for hunk operations
- Help system documentation for advanced features
- Performance optimizations and polish

### Advanced Features (Beyond Phase 6)
- **Line-level Staging**: Stage individual lines within hunks
- **Smart Merging**: Intelligent conflict resolution
- **Hunk Editing**: Edit hunks before staging
- **Syntax Highlighting**: Color-coded diff content

This detailed plan provides a clear roadmap for implementing Magit's most powerful feature while maintaining the project's goals of performance and usability.