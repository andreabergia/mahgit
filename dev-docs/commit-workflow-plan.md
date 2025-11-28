# Commit Modal Implementation Plan

## Overview
Implement a modal-based commit workflow with multi-key sequences, supporting fast-typing (e.g., "cc") and deliberate selection via a timeout-based modal popup.

## User Experience

### Fast Typing Flow
```
User: cc (typed quickly within 300ms)
→ Opens editor immediately for normal commit
→ No modal shown
```

### Deliberate Selection Flow
```
User: c (waits)
→ After 300ms, modal appears at bottom showing:
   ┌─ Commit ─────────────────────────┐
   │ c  commit          Create commit │
   │ a  amend           Amend HEAD    │
   │ e  extend          Extend HEAD   │
   │ w  reword          Reword HEAD   │
   └──────────────────────────────────┘
User: a
→ Modal closes, opens editor for amend commit
```

## Architecture Components

### 1. Modal System (`src/ui/modal.rs`)

**ModalContext Enum**
```rust
pub enum ModalContext {
    None,
    Commit,
    // Future: Branch, Push, Pull, Log, etc.
}
```

**Modal Trait**
```rust
pub trait Modal {
    /// Get the modal title
    fn title(&self) -> &str;

    /// Get the available options as (key, short_label, description)
    fn options(&self) -> Vec<(char, &str, &str)>;

    /// Handle a key press, return command to execute
    fn handle_key(&self, key: char) -> Option<Command>;

    /// Render the modal in the given area
    fn render(&self, frame: &mut Frame, area: Rect);
}
```

**Generic Bottom Panel Renderer**
- Reusable rendering function for bottom-panel modals
- Styled consistently with help overlay
- Auto-sized based on content

### 2. Commit Modal (`src/ui/modals/commit.rs`)

**CommitModal Struct**
```rust
pub struct CommitModal {
    config: Arc<Config>, // For theme access
}

impl Modal for CommitModal {
    fn title(&self) -> &str { "Commit" }

    fn options(&self) -> Vec<(char, &str, &str)> {
        vec![
            ('c', "commit", "Create commit"),
            ('a', "amend", "Amend HEAD"),
            ('e', "extend", "Extend HEAD"),
            ('w', "reword", "Reword HEAD"),
        ]
    }

    fn handle_key(&self, key: char) -> Option<Command> {
        match key {
            'c' => Some(Command::Commit(CommitMode::Normal)),
            'a' => Some(Command::Commit(CommitMode::Amend)),
            'e' => Some(Command::Commit(CommitMode::Extend)),
            'w' => Some(Command::Commit(CommitMode::Reword)),
            _ => None,
        }
    }
}
```

### 3. Command Extension (`src/ui/input.rs`)

**New Command Variants**
```rust
pub enum Command {
    // ... existing commands ...

    /// Open commit modal (or execute immediately if second key pressed fast)
    OpenCommitModal,

    /// Execute a commit operation
    Commit(CommitMode),
}

#[derive(Debug, Clone, Copy)]
pub enum CommitMode {
    Normal,      // git commit
    Amend,       // git commit --amend
    Extend,      // git commit --amend --no-edit
    Reword,      // git commit --amend (message only)
}
```

### 4. Input Handler Enhancement

**PrefixKeyState**
```rust
struct PrefixKeyState {
    prefix_char: char,
    timestamp: Instant,
    timeout_ms: u64, // 300ms default
}
```

**Enhanced Two-Key Handling**
- Extend existing `handle_two_key_sequence()` logic
- Support prefix keys: 'c' (commit), future: 'b' (branch), 'P' (push), etc.
- Track timeout for modal display
- Fast-path for quick key sequences

**Key Processing Flow**
```rust
fn handle_key_event(&mut self, key: KeyEvent, elapsed_since_prefix: Option<Duration>)
    -> InputResult {

    // Check if we have a pending prefix
    if let Some(prefix) = self.previous_key {
        let second_char = extract_char(key);

        // Check if we should show modal (timeout reached)
        if elapsed_since_prefix >= Duration::from_millis(300) {
            return InputResult::ShowModal(prefix.key);
        }

        // Fast path: execute command immediately
        if let Some(cmd) = resolve_two_key_command(prefix.key, second_char) {
            self.previous_key = None;
            return InputResult::Command(cmd);
        }
    }

    // Check if this is a prefix key
    if is_prefix_key(key) {
        self.previous_key = Some(PreviousKey { key, timestamp: Instant::now() });
        return InputResult::Pending;
    }

    // Normal single-key command
    map_single_key_to_command(key)
}

enum InputResult {
    Command(Command),
    ShowModal(char), // Show modal for this prefix
    Pending,         // Waiting for second key
    None,
}
```

### 5. App State Extension (`src/ui/mod.rs`)

**New Fields**
```rust
pub struct App {
    // ... existing fields ...

    /// Currently active modal (if any)
    active_modal: Option<Box<dyn Modal>>,

    /// Modal context for tracking what modal system is active
    modal_context: ModalContext,
}
```

**Modal Management Methods**
```rust
impl App {
    fn show_commit_modal(&mut self) {
        self.active_modal = Some(Box::new(CommitModal::new(self.config.clone())));
        self.modal_context = ModalContext::Commit;
    }

    fn close_modal(&mut self) {
        self.active_modal = None;
        self.modal_context = ModalContext::None;
    }

    fn handle_modal_input(&mut self, key: char) -> Result<()> {
        if let Some(modal) = &self.active_modal {
            if let Some(cmd) = modal.handle_key(key) {
                self.close_modal();
                self.handle_command(cmd)?;
            }
        }
        Ok(())
    }
}
```

### 6. Event Loop Integration

**Modified Event Loop**
```rust
// In App::run_event_loop()
loop {
    // ... render frame ...

    let event = read_event()?;

    // If modal is active, route keys to modal first
    if self.modal_context != ModalContext::None {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Esc => self.close_modal(),
                KeyCode::Char(c) => self.handle_modal_input(c)?,
                _ => {} // Ignore other keys in modal mode
            }
            continue;
        }
    }

    // Normal input handling
    let input_result = self.input_handler.handle_event(event);

    match input_result {
        InputResult::Command(cmd) => self.handle_command(cmd)?,
        InputResult::ShowModal(prefix) => {
            match prefix {
                'c' => self.show_commit_modal(),
                _ => {} // Future: other modals
            }
        }
        InputResult::Pending => {
            // Check if timeout reached on next iteration
        }
        InputResult::None => {}
    }
}
```

### 7. Rendering Integration

**Modal Rendering Layer**
```rust
// In App::render()
fn render(&mut self, frame: &mut Frame) {
    // 1. Render main status view
    self.render_status(frame, main_area);

    // 2. Render feedback message (if any)
    self.render_feedback(frame, bottom_area);

    // 3. Render help overlay (if shown)
    if self.show_help {
        self.render_help(frame);
    }

    // 4. Render active modal (if any) - renders on top
    if let Some(modal) = &self.active_modal {
        modal.render(frame, modal_area);
    }
}
```

**Bottom Panel Modal Style**
- Similar to help overlay
- Clear background widget
- Block with borders and title
- Three-column layout: `key | label | description`
- Height auto-calculated based on number of options
- Positioned at bottom, above feedback message

### 8. Git Commit Operations (`src/operations/commit.rs`)

**New File for Commit Operations**
```rust
pub struct CommitOperations<'repo> {
    repository: &'repo Repository,
}

impl<'repo> CommitOperations<'repo> {
    pub fn prepare_commit(&self, mode: CommitMode) -> Result<CommitPreparation> {
        match mode {
            CommitMode::Normal => self.prepare_normal_commit(),
            CommitMode::Amend => self.prepare_amend_commit(),
            CommitMode::Extend => self.prepare_extend_commit(),
            CommitMode::Reword => self.prepare_reword_commit(),
        }
    }

    pub fn execute_commit(&self, preparation: CommitPreparation) -> Result<Oid> {
        // Execute the actual commit with libgit2
        // Handle --amend, --no-edit flags, etc.
    }
}

pub struct CommitPreparation {
    pub mode: CommitMode,
    pub initial_message: Option<String>, // For amend/reword: existing HEAD message
    pub flags: CommitFlags,
}
```

### 9. Editor Integration

**Reuse Existing Mechanism**
- Use `App::pending_editor_file` field
- Create temp file with initial commit message (if any)
- Set `pending_editor_file = Some(temp_path)`
- Existing event loop already handles editor spawning
- Read message back after editor closes
- Execute commit with message

**Commit Message Handling**
- Normal: Start with empty message
- Amend: Pre-populate with HEAD commit message
- Extend: Skip editor entirely (--no-edit)
- Reword: Pre-populate with HEAD commit message

## Implementation Order

### Phase 1: Core Modal System
1. Create `src/ui/modal.rs` with Modal trait and ModalContext enum
2. Create `src/ui/modals/` directory and `commit.rs` with CommitModal
3. Add modal rendering helper functions

### Phase 2: Command & Input System
4. Extend Command enum with Commit variants
5. Enhance InputHandler with prefix key timeout logic
6. Add input routing for modal vs normal mode

### Phase 3: App Integration
7. Add modal state fields to App
8. Implement modal management methods
9. Integrate modal rendering into App::render()
10. Update event loop for modal handling

### Phase 4: Git Operations
11. Create `src/operations/commit.rs`
12. Implement commit preparation logic for each mode
13. Implement commit execution with libgit2

### Phase 5: Editor Integration & Testing
14. Connect commit commands to editor spawning
15. Handle commit message reading and execution
16. Test fast-typing "cc" flow
17. Test deliberate modal selection flow
18. Test amend commit at minimum

## Files to Create
- `src/ui/modal.rs` - Generic modal system
- `src/ui/modals/mod.rs` - Modal module exports
- `src/ui/modals/commit.rs` - Commit modal implementation
- `src/operations/commit.rs` - Git commit operations

## Files to Modify
- `src/ui/input.rs` - Add Command variants, enhance InputHandler
- `src/ui/mod.rs` - Add modal state, rendering, event handling
- `src/operations/mod.rs` - Export commit operations module
- `src/main.rs` - Potentially for testing

## Testing Strategy

### Manual Testing
1. Fast-typing: Type "cc" quickly → editor opens immediately
2. Deliberate: Press "c", wait, see modal, press "c" → editor opens
3. Amend: Type "ca" quickly → editor opens with HEAD message
4. Modal dismiss: Press "c", wait, press Esc → modal closes
5. Full commit flow: Stage files, "cc", write message, save, see commit feedback

### Edge Cases
- Pressing invalid second key after 'c'
- Switching between files while modal is open
- Multiple rapid prefix keys
- Timeout boundary conditions (just before/after 300ms)

## Future Extensions
- Branch modal with 'b' prefix
- Push modal with 'P' prefix
- Pull modal with 'F' prefix
- Log modal with 'l' prefix
- Reusable modal system for all command families
- Configurable keybindings per modal
- Configurable timeout duration
