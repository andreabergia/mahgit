# Phase 1: Repository Foundation - Detailed Implementation Plan

**Core Objective**: Establish basic Git repository interaction and display repository state (excluding submodules and ignored files)

## Implementation Tasks

### 1. Project Setup
- Initialize `Cargo.toml` with core dependencies: `git2`, `crossterm`, `ratatui`
- Set up basic `main.rs` with error handling framework
- Create modular structure: `repository.rs`, `status.rs`, `display.rs`

### 2. Repository Discovery & Validation
- Implement `find_repository()` function using `git2::Repository::discover()`
- Create `Repository` struct wrapping `git2::Repository`
- Add validation for repository integrity and accessibility
- Handle common failure cases: not a repo, bare repo, corrupted repo

### 3. Basic File Status Analysis
- Implement `get_repository_status()` using `git2::Repository::statuses()`
- Categorize files into **4 basic categories only**:
  - **Staged**: ready for commit (index changes)
  - **Unstaged**: working directory changes  
  - **Untracked**: new files not in Git
  - **Conflicted**: merge conflict state
- Skip submodules and ignored files for now

### 4. Simple Status Display
- Create terminal output showing the 4 file categories
- Format similar to `git status` with clear section headers
- Show summary counts for each category
- Display current branch name and basic repository info

### 5. Error Handling Strategy
- Custom error types for different failure modes
- User-friendly messages: "Not in a Git repository", "Repository corrupted"
- Exit codes that match Git conventions
- Graceful failure outside Git repos

## Testing Strategy

### Test Structure Overview
```
tests/
├── unit/                    # Fast unit tests
│   ├── repository_test.rs   # Repository logic tests
│   └── display_test.rs      # Display formatting tests
├── integration/             # Git operation tests
│   └── git_operations_test.rs
└── fixtures/                # Test repository setups
    ├── clean_repo/
    ├── dirty_repo/ 
    └── conflict_repo/
```

### 1. Unit Tests (Fast, No Git Dependencies)
**Test repository error handling and validation logic:**
- `RepositoryError` display formatting
- Repository state categorization logic
- Status display formatting with mock data
- Edge cases for file path handling

```rust
// Example test structure
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_repository_status_is_clean() {
        let status = RepositoryStatus {
            branch_name: "main".to_string(),
            staged: vec![],
            unstaged: vec![],
            untracked: vec![],
            conflicted: vec![],
        };
        assert!(status.is_clean());
    }
}
```

### 2. Integration Tests (Git Operations)
**Test actual Git interactions with temporary repositories:**
- Repository discovery in various directory structures
- Status detection for different file states
- Error handling for invalid repositories
- Branch name detection

### 3. Test Repository Fixtures
**Create programmatic test repositories:**
```rust
// Helper function to create test scenarios
fn create_test_repo_with_changes() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo = git2::Repository::init(&temp).unwrap();
    
    // Set up initial commit
    // Add files in different states
    // Return temp directory
}
```

### 4. TDD Approach (Flexible)
**Red-Green-Refactor cycle for key functions:**

1. **Start with error cases** (easiest to test):
   - Test "not in repository" error
   - Test invalid repository handling
   
2. **Test clean repository state**:
   - Empty repository
   - Repository with committed files only

3. **Add complexity gradually**:
   - Untracked files
   - Staged changes
   - Unstaged changes
   - Conflict states

### 5. Test Categories by Priority

**High Priority (Phase 1)**:
- Repository discovery and validation
- Basic status categorization
- Error message formatting
- Clean repository detection

**Medium Priority**:
- Complex repository states
- Performance with many files
- Cross-platform path handling

**Future Phases**:
- UI interaction tests
- Diff parsing accuracy
- Keyboard navigation

### 6. Testing Tools
- **`tempfile`** crate for temporary test repositories
- **`git2`** for programmatic Git setup in tests
- **`assert_matches`** for pattern matching on errors
- Standard Rust `#[test]` and `cargo test`

### Practical TDD Workflow
1. Write failing test for next feature
2. Implement minimal code to pass
3. Refactor with confidence
4. Add integration test for full flow
5. Move to next feature

## Success Validation
- Works in any valid Git repository
- Shows meaningful status for clean/dirty repos
- Handles merge conflicts appropriately
- Fails gracefully outside Git repos
- Basic performance acceptable for typical repositories

This simplified foundation focuses on the core Git integration needed before moving to the interactive interface in Phase 2.