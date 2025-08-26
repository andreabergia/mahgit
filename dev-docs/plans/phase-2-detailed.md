# Phase 2: Status Buffer Interface - Detailed Implementation Plan

## Overview
Transform the current console-based status display into an interactive terminal UI that matches Magit's status buffer behavior. This phase builds on the existing repository status collection (`src/status.rs`) and replaces the simple console output (`src/display.rs`) with a keyboard-navigable interface.

## Current State Analysis
**Existing Foundation:**
- ✅ Repository discovery and validation (`src/repository.rs`)
- ✅ Status categorization (staged, unstaged, untracked, conflicted) (`src/status.rs:5-67`)
- ✅ Basic console output (`src/display.rs:3-42`)
- ✅ Core dependencies: `ratatui`, `crossterm`, `git2`

**Gaps to Address:**
- No terminal UI event loop or rendering
- No keyboard input handling
- No navigation state management
- No interactive visual feedback
- No layout management for different terminal sizes

## Implementation Tasks

### Task 2.1: Terminal UI Foundation
**File:** `src/ui/mod.rs` (new module)
**Dependencies:** `ratatui`, `crossterm`

**Objectives:**
- Initialize terminal with proper setup/cleanup
- Establish main event loop architecture
- Handle terminal resize events
- Implement graceful shutdown on Ctrl+C

**Key Components:**
```rust
pub struct App {
    should_quit: bool,
    current_view: ViewType,
    // State management fields
}

pub enum ViewType {
    Status,
    // Future: Diff, Log, etc.
}
```

**Success Criteria:**
- Application launches into terminal UI mode
- Graceful exit with terminal restoration
- Handles terminal resize without crashes
- Event loop processes input without blocking

### Task 2.2: Navigation State Management
**File:** `src/ui/navigation.rs` (new)

**Objectives:**
- Track current selection across different file categories
- Handle navigation boundaries (top/bottom of lists, empty sections)
- Maintain selection when repository status updates
- Provide selection change notifications for UI updates

**Key Components:**
```rust
pub struct NavigationState {
    current_section: StatusSection,
    selected_index: usize,
    sections: Vec<StatusSection>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum StatusSection {
    Staged,
    Unstaged,
    Untracked,
    Conflicted,
}
```

**Navigation Logic:**
- Arrow Up/Down: Move within current section
- Section boundaries: Auto-jump to next/previous non-empty section
- Home/End: Jump to first/last item globally
- Page Up/Down: Scroll by terminal height

**Success Criteria:**
- Smooth navigation between all file categories
- Selection persists across status updates
- Handles empty sections gracefully
- Visual feedback matches current selection

### Task 2.3: Status Buffer Layout System
**File:** `src/ui/status_view.rs` (new)

**Objectives:**
- Render repository status in hierarchical sections
- Implement responsive layout for different terminal sizes
- Match Magit's visual organization and information density
- Handle long filenames and paths gracefully

**Layout Structure:**
```
╭─ Repository: /path/to/repo (branch: main) ─╮
│                                           │
│ Staged changes (2)                        │
│ > modified   src/main.rs                  │
│   new        src/ui/mod.rs                │
│                                           │
│ Unstaged changes (1)                      │
│   modified   README.md                    │
│                                           │
│ Untracked files (1)                       │
│   new        temp.log                     │
╰───────────────────────────────────────────╯
```

**Visual Design Elements:**
- Section headers with file counts
- Indented file listings with status indicators
- Current selection highlight
- Consistent spacing and alignment
- Truncation strategy for long paths

**Success Criteria:**
- Clear visual hierarchy between sections
- Readable on 80x24 terminal minimum
- Selection highlight is obvious
- File paths display meaningfully even when truncated

### Task 2.4: Keyboard Input Handler
**File:** `src/ui/input.rs` (new)

**Objectives:**
- Process keyboard events and route to appropriate handlers
- Implement Magit-style key bindings
- Provide contextual key binding behavior
- Handle edge cases (rapid input, invalid keys)

**Key Bindings (Phase 2 subset):**
```
Navigation:
- j/↓: Move down
- k/↑: Move up  
- gg/Home: Jump to top
- G/End: Jump to bottom

Application:
- q: Quit application
- C-c: Force quit
- r: Refresh repository status

Help:
- ?: Show key binding help (future)
```

**Input Processing Flow:**
1. Capture key event via crossterm
2. Convert to internal command enum
3. Route to appropriate handler based on current view
4. Update application state
5. Trigger UI refresh

**Success Criteria:**
- All key bindings work reliably
- No input lag or dropped keystrokes
- Intuitive key binding layout
- Graceful handling of unmapped keys

### Task 2.5: Integration and Refactoring
**Files:** `src/main.rs`, `src/display.rs` (modify existing)

**Objectives:**
- Replace console output with terminal UI
- Integrate new UI components with existing status logic
- Ensure smooth startup and shutdown
- Maintain error handling from Phase 1

**Refactoring Tasks:**
- Move `src/display.rs` → `src/ui/console.rs` (for debugging/fallback)
- Update `main.rs` to launch UI event loop instead of one-shot display
- Add command-line flag for console vs UI mode (development aid)
- Preserve existing error handling and exit codes

**Integration Points:**
```rust
// In main.rs
fn run() -> Result<(), RepositoryError> {
    let repo = Repository::discover(".")?;
    let status = RepositoryStatus::new(&repo)?;
    
    // NEW: Launch UI instead of console output
    let mut app = App::new(status);
    app.run()?; // Event loop replaces display_status()
    
    Ok(())
}
```

**Success Criteria:**
- Application launches directly into interactive mode
- All existing error handling preserved
- Performance comparable to console version
- No regressions in repository status accuracy

## Testing Strategy

### Unit Tests
- Navigation state transitions (`src/ui/navigation.rs`)
- Key binding parsing and routing (`src/ui/input.rs`)
- Layout calculations for different terminal sizes

### Integration Tests
- Full UI workflow: launch → navigate → quit
- Status updates while UI is running
- Terminal resize handling
- Error recovery scenarios

### Manual Testing Scenarios
- Repository with all status types (staged, unstaged, untracked, conflicted)
- Empty repository (clean working tree)
- Large number of files (performance testing)
- Very long file paths (layout testing)
- Narrow terminal windows (80 columns, 24 rows)

## File Structure After Phase 2
```
src/
├── main.rs                 (updated: UI launch)
├── lib.rs                  (updated: expose UI modules)
├── repository.rs           (unchanged)
├── status.rs              (unchanged)
├── display.rs             (deprecated → ui/console.rs)
└── ui/
    ├── mod.rs             (new: UI foundation)
    ├── navigation.rs      (new: selection state)
    ├── status_view.rs     (new: layout rendering)
    ├── input.rs          (new: keyboard handling)
    └── console.rs        (moved: fallback display)
```

## Success Metrics
**Functional Goals:**
- Navigate through all file categories with arrow keys
- Clear visual indication of current selection  
- Sections clearly distinguishable with headers and spacing
- Interface remains usable on minimum terminal size (80x24)
- Responsive to user input without lag

**Performance Goals:**
- UI refresh < 16ms for smooth interaction
- Memory usage similar to Phase 1 (no significant increase)
- Startup time < 100ms for typical repositories

**User Experience Goals:**
- Navigation feels natural and predictable
- Visual design is clean and uncluttered
- Information density matches Magit status buffer
- Error states are clearly communicated

## Risk Mitigation
**Terminal Compatibility:** Test across macOS Terminal, iTerm2, Linux terminals
**Performance with Large Repos:** Implement lazy loading if file counts > 1000
**Layout Edge Cases:** Comprehensive testing with various path lengths and terminal sizes
**Input Handling:** Robust event processing to prevent UI lock-ups

## Dependencies for Future Phases
This phase establishes the UI architecture that will be extended in Phase 3 (File Operations) and Phase 4 (Diff Viewer). The navigation system and layout framework designed here will support:
- Interactive file operations (staging/unstaging)
- Diff view integration
- Multi-view management
- Command palette/help system