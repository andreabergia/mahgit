# Accordion UI - Test Coverage Analysis

**Date:** 2025-10-18
**Analyzed By:** test-coverage-analyzer agent
**Manual Test Report:** ACCORDION_UI_TEST_REPORT.md

## Executive Summary

The accordion UI feature has **88 passing automated tests** (75 unit + 13 integration) with excellent coverage for input handling and navigation logic. **Progress: 2/20 accordion-specific tests implemented** (Tests #1 & #3 merged: Inline Diff Expansion + Content Correctness ✅). Remaining critical gaps exist in testing multiple diffs, section management, and viewport tracking.

## Current Test Coverage

### Existing Automated Tests ✓

**Unit Tests (75 tests in src/):**
- Input handling and key event processing
- Navigation state management
- File tree operations
- Diff computation logic
- Selection tracking

**Integration Tests (12 tests in tests/):**
- Basic UI interactions
- Repository operations
- File selection and navigation
- Status display

### Test Infrastructure ✓

Well-designed and ready for expansion:
- `TestApp` - Simulates full application state
- `TestBackend` - Captures UI rendering output
- `create_test_repository()` - Sets up Git repos with commits
- Helper functions for key simulation and state assertions

## Critical Coverage Gaps

### Priority 1: Critical Accordion Features (8 tests needed)

#### 1. **Inline Diff Expansion/Collapse**
- **Status:** ✅ TESTED (test_inline_diff_expansion_and_content)
- **Risk:** HIGH - Core feature
- **Location:** `src/ui/status_view.rs:168-183, 253-268`
- **Test:** `tests/user_interface_test.rs:387-492`
- **Coverage:** Verifies that pressing Tab on a file expands inline diffs with actual file content

#### 2. **Multiple Diffs Expanded Simultaneously**
- **Status:** ❌ Not tested
- **Risk:** HIGH - Multi-accordion state management
- **Gap:** Cannot verify that expanding file B while A is expanded maintains both states
- **Test Needed:** Expand multiple files, verify all remain expanded

#### 3. **Diff Content Correctness**
- **Status:** ✅ TESTED (test_inline_diff_expansion_and_content)
- **Risk:** HIGH - Data integrity
- **Test:** `tests/user_interface_test.rs:387-492` (merged with test #1)
- **Coverage:** Verifies displayed diff content matches actual file changes (additions, deletions, context lines)

#### 4. **Section Collapse/Expand**
- **Status:** ❌ Infrastructure exists but completely untested
- **Location:** `src/ui/file_tree.rs:265-310` (rendering), `src/app_state.rs:116-127` (toggling)
- **Gap:** Section headers render but collapse/expand behavior never verified
- **Test Needed:** Toggle sections, verify visibility changes

#### 5. **Tab Key Context-Aware Toggling**
- **Status:** ⚠️ Unit tested, not integration tested
- **Location:** `src/input_handler.rs:219-236`
- **Gap:** Logic tested in isolation, not verified in full UI context
- **Test Needed:** Tab on file vs non-file item, verify correct behavior

#### 6. **Enter Key Expanding**
- **Status:** ❌ Not tested
- **Gap:** Selection-based diff expansion not verified
- **Test Needed:** Press Enter on collapsed file, verify expansion

#### 7. **Backspace Collapsing**
- **Status:** ❌ Not tested
- **Gap:** Single-key collapse not verified
- **Test Needed:** Press Backspace on expanded file, verify collapse

#### 8. **Section Traversal Navigation**
- **Status:** ❌ Not tested
- **Gap:** Navigation between collapsed sections not verified
- **Test Needed:** Collapse sections, navigate with j/k, verify skipping

### Priority 2: Important Supporting Features (6 tests needed)

#### 9. **Viewport Scroll Tracking**
- **Status:** ❌ Not tested
- **Location:** `src/ui/file_tree.rs:1043-1063`
- **Gap:** Auto-scroll to keep selected items visible not verified
- **Test Needed:** Expand large diff, verify viewport adjusts

#### 10. **Diff Caching Behavior**
- **Status:** ❌ Not tested
- **Location:** `src/diff_cache.rs`
- **Gap:** Cache hits, misses, and invalidation not verified
- **Test Needed:** Expand same file twice, verify cache hit on second

#### 11. **Selection Maintenance During Expand/Collapse**
- **Status:** ❌ Not tested
- **Gap:** Selection tracking during state changes not verified
- **Test Needed:** Expand file, verify selection stays on same item

#### 12. **Empty File Diff Edge Case**
- **Status:** ❌ Not tested
- **Gap:** Expanding files with no changes not tested
- **Test Needed:** Toggle empty diff, verify graceful handling

#### 13. **Large File Diff Performance**
- **Status:** ❌ Not tested
- **Gap:** Performance with large diffs not validated
- **Test Needed:** Expand file with 1000+ line diff, verify reasonable performance

#### 14. **Rapid Toggle Operations**
- **Status:** ❌ Not tested
- **Gap:** Quick expand/collapse sequences not tested
- **Test Needed:** Rapidly toggle same file, verify state consistency

### Priority 3: Nice-to-Have Enhancements (6 tests needed)

#### 15. **Diff Loading State Feedback**
- **Status:** ❌ Not implemented or tested
- **Gap:** No loading indicators for slow diff operations
- **Test Needed:** Mock slow diff, verify loading state shows

#### 16. **Diff Generation Error Handling**
- **Status:** ⚠️ Basic error handling exists, not thoroughly tested
- **Gap:** Edge cases like binary files, permission errors not verified
- **Test Needed:** Attempt to expand binary file, verify graceful error

#### 17. **Binary File Diff Handling**
- **Status:** ❌ Not tested
- **Gap:** Binary file detection and messaging not verified
- **Test Needed:** Toggle binary file, verify appropriate message

#### 18. **Mouse Click Expansion**
- **Status:** ❌ Not implemented
- **Gap:** Mouse integration not present
- **Test Needed:** (Future) Click file to expand

#### 19. **Keyboard Shortcut Consistency**
- **Status:** ⚠️ Documented but not integration tested
- **Gap:** Full shortcut suite not verified end-to-end
- **Test Needed:** Verify all documented shortcuts work

#### 20. **Memory Usage with Many Expanded Diffs**
- **Status:** ❌ Not tested
- **Gap:** Memory behavior with many diffs not monitored
- **Test Needed:** (Future) Expand 50+ files, verify memory stays reasonable

## Hunk Navigation - Not Implemented

The following features from the manual test report are **completely unimplemented**:
- Hunk-level expansion (Tab on hunk)
- Hunk-level collapse (Backspace on hunk)
- Hunk navigation (n/N keys)
- Stage/unstage hunks (s/u keys)

**No tests needed yet** - implement features first.

## Recommended Test Implementation Plan

### Phase 1: Core Accordion Validation (Priority 1)
**Estimated Effort:** 2-3 hours
**Location:** Add to `tests/user_interface_test.rs`

Implement tests 1-8 to validate the core accordion functionality:
```rust
#[test]
fn test_inline_diff_expansion() {
    // Create test repo with modified file
    // Select file, press Tab
    // Assert diff content appears in rendered buffer
}

#[test]
fn test_multiple_diffs_expanded() {
    // Expand file A, expand file B
    // Assert both remain expanded
}

#[test]
fn test_diff_content_correctness() {
    // Create known file changes
    // Expand diff
    // Assert rendered content matches expected diff output
}

#[test]
fn test_section_collapse_expand() {
    // Toggle section
    // Assert items visibility changes
}
```

### Phase 2: Supporting Features (Priority 2)
**Estimated Effort:** 2-3 hours
**Location:** Add to `tests/user_interface_test.rs`

Implement tests 9-14 for viewport, caching, and edge cases.

### Phase 3: Enhancements (Priority 3)
**Estimated Effort:** 1-2 hours
**Location:** Add to `tests/user_interface_test.rs`

Implement tests 15-20 as time permits.

## Implementation Notes

### Leveraging Existing Infrastructure

The existing test setup is well-suited for these tests:

```rust
// Pattern from existing tests
let (mut app, _temp_dir) = create_test_app_with_modified_file();
app.handle_key(KeyCode::Char('j')); // Navigate
app.handle_key(KeyCode::Tab);       // Expand
let buffer = TestBackend::render(&app);
assert!(buffer.contains("expected diff content"));
```

### No Major Refactoring Required

The codebase is already testable:
- ✓ Logic separated from UI rendering
- ✓ Test helpers for state setup
- ✓ Deterministic test repositories
- ✓ Mockable backend for UI capture

### Test Data Strategy

Use `create_test_repository()` pattern:
```rust
fn create_repo_with_large_diff() -> (TempDir, Repository) {
    let temp = TempDir::new().unwrap();
    let repo = Repository::init(&temp).unwrap();
    // Create file with 1000+ lines changed
    // ...
}
```

## Confidence Assessment

| Aspect | Confidence Level | Notes |
|--------|-----------------|-------|
| Core navigation | ✓✓✓ High | Well tested with 75+ unit tests |
| Input handling | ✓✓✓ High | Comprehensive keyboard event tests |
| Basic UI rendering | ✓✓ Medium | Integration tests exist but limited |
| **Accordion expand/collapse** | ✗ **Low** | **No integration tests** |
| **Section management** | ✗ **Low** | **Infrastructure untested** |
| **Diff content accuracy** | ✗ **Low** | **Not verified** |
| Viewport tracking | ✓ Medium | Logic exists, not tested |
| Performance | ? Unknown | No performance tests |

## Conclusion

The accordion UI feature has **solid foundational testing** for navigation and input handling, but **lacks critical integration tests** for the accordion-specific behaviors that make it valuable. The existing test infrastructure is excellent and ready to support comprehensive accordion testing.

**Immediate Action Required:** Implement the 8 Priority 1 tests to validate core accordion functionality before considering the feature production-ready.

## References

- Manual Test Report: `dev-docs/ACCORDION_UI_TEST_REPORT.md`
- Existing Tests: `tests/user_interface_test.rs`
- Core Implementation: `src/ui/file_tree.rs:1043-1289`
- Input Handling: `src/input_handler.rs:219-236`
- Section Management: `src/app_state.rs:116-127`
- Diff Caching: `src/diff_cache.rs`
