# Log Feature — Review Fixes Plan

Findings from a correctness / completeness / test-coverage review of the
`feat/log-viewer` branch. Items are ordered by priority. Each fix should be
accompanied by automated tests (unit/integration); manual smoke testing is done
separately.

## P1 — Correctness bugs

### 1. Binary files in commit diffs are broken
- `src/diff/generator.rs` (`generate_commit_file_diff`, ~line 165): returns
  `Err(DiffError::BinaryFile)` for binary files.
- `src/ui/mod.rs` (`toggle_log_expansion`, File branch): treats that error as a
  generic "Failed to load diff" toast, so the file never expands.
- Consequence: the `diff.binary` "Binary file (not shown)" branch in
  `src/ui/log_view.rs:305-313` is unreachable dead code.
- Fix: in the File-expansion error arm, match `DiffError::BinaryFile` and store
  `Diff { binary: true, hunks: vec![], .. }` so the file expands and renders the
  graceful message. Consider the same treatment for
  `DiffError::TerminalCompatibility`.

### 2. `format_relative_time` can panic the render loop
- `src/ui/log_view.rs:369-372`: `duration_since(UNIX_EPOCH).unwrap()` panics if
  the system clock is before the Unix epoch — and it runs on every render.
- Fix: use `unwrap_or_default()` (or map to `0`).

### 3. Pagination unreachable except via single-step `j`
- `src/ui/mod.rs` (`handle_log_command`): load-more only fires in the
  `MoveDown | ScrollDiffDown` arm. `G` (MoveToBottom), PageDown, and viewport
  scroll never paginate, so `G` stops at the first batch (commit #49).
- Fix: run the load-more check after `MoveToBottom` and `PageDiffDown` as well
  (or drive load-more off the rendered viewport reaching `has_more`).

## P2 — Correctness / state polish

### 4. Commit expand/collapse doesn't clear manual scroll
- `src/ui/mod.rs` (`toggle_log_expansion`, Commit branch): toggles `expanded`
  but never calls `clear_manual_scroll` (file/hunk collapse already do).
- Effect: after a manual scroll, the cursor highlight isn't re-synced until an
  arrow key.
- Fix: call `clear_manual_scroll` when commit expansion toggles.

### 5. Re-clamp scroll offset when content shrinks
- `src/ui/log_navigation.rs` / `update_viewport_metrics`: `scroll_offset` is not
  re-clamped against a freshly computed `max_scroll_offset`, so collapsing a
  large commit can briefly show a blank viewport (self-corrects next key).
- Fix: re-clamp `scroll_offset` inside `update_viewport_metrics`.

### 6. Call `ensure_cursor_valid` in the Log render path
- `src/ui/mod.rs` render: only Status calls `ensure_cursor_valid`. No live bug
  today (entries only grow), but a safety gap if reload/trim is added later.
- Fix: call it in the Log render arm.

## P3 — Completeness gaps (Magit parity + codebase consistency)

### 7. View-aware help overlay
- `src/ui/input.rs` (`get_help_text`) + `src/ui/mod.rs` (`render_help_overlay`):
  help is a single static status-view list. It never documents the log view and
  never says how to exit (`q`/`Esc`), while advertising status-only keys that
  no-op in the log view.
- Fix: route help text per current view.

### 8. Dead keys in the log view
- `src/ui/mod.rs` (`handle_log_command` `_ => {}` arm): `r` (refresh), `n`/`p`,
  `=`/`-`, `w`/`W`, `Enter` are silently ignored.
- Fix (minimum): implement `r` (reload the log) — a core Magit affordance.
- Larger: `Enter` to open a commit-detail view (full message body, author/
  committer, full hash, parents). Currently only `summary` is stored on
  `LogEntry`.

### 9. Loading feedback for large synchronous diff loads
- `toggle_log_expansion` loads diffs from git2 synchronously with no feedback;
  large commits block the UI thread silently. Other ops use `FeedbackManager`.

### Deferred / nice-to-have (not scheduled)
- Ref/branch/tag decorations on commits.
- Commit graph / topology.
- Copy-hash / yank action.
- Jump-to-commit / search / filter.
- Log scope beyond HEAD (modal scaffold already supports growth).
- Display committer time vs author time consistency.

## P1 (tests) — Backfill the untested log logic

Reuse existing fixtures: `setup_test_repo()` (`src/diff/generator.rs`),
`create_test_repo_with_initial_commit` / `create_test_file` /
`commit_then_modify_workdir` (`src/ui/mod.rs` test module), and the
`sample_hunk` / `sample_diff` / `sample_expansion` helpers
(`src/ui/log_navigation.rs`).

### Repository (`src/repository.rs`, currently untested)
- `load_log_entries`: empty repo → `Err`; single commit → 1 entry,
  `has_more == false`; `count < total` → `entries.len() == count`,
  `has_more == true`; `skip` window correctness (off-by-one prone); field
  mapping (7-char `short_hash`, summary/author populated).
- `changed_files_for_commit`: normal vs first parent; root commit (all `Added`);
  merge commit (first-parent only).

### Diff generator (`src/diff/generator.rs::generate_commit_file_diff`)
- Root commit → full-file addition diff.
- Binary file → `Err(DiffError::BinaryFile)` (update once fix #1 lands).
- Unchanged path → empty hunks, no panic.

## P2 (tests) — Navigation & wiring

### Navigation (`src/ui/log_navigation.rs`)
- `next_cursor`/`previous_cursor` round-trip (collapsed and expanded).
- Bounds: stay put at top and bottom.
- `move_to_bottom` / `last_item_of_commit` lands on deepest visible item.
- `move_up_hierarchy`: Hunk → File → Commit → no-op.
- `page_up`/`page_down` clamp at bounds.
- `ensure_cursor_valid`: clamp past-end cursor; empty data → `None`; `None` on
  non-empty → `Commit{0}`.
- Manual-scroll flag clears after any move.
- Extend `cursor_flat_index` test to cover the multi-hunk separator line and a
  File-level cursor.

### Input (`src/ui/input.rs`)
- `ll` sequence test mirroring `test_gg_sequence` (first `l` → `Pending`;
  `ll` → `Command::OpenLog`).

### App wiring (`src/ui/mod.rs`)
- `open_log_view` / `close_log_view` set/clear view + state.
- `toggle_log_expansion`: load-once then collapse.
- `load_more_log_entries`: no-op when `has_more == false`; appends + advances
  `total_loaded` otherwise (needs > `INITIAL_BATCH` commits — note setup caveat).

## Suggested execution order
1. Fixes #1, #2, #3 (P1 correctness).
2. Fixes #4, #5, #6 (state polish).
3. P1 tests (repository + diff generator).
4. Fix #7, #8 (help + refresh).
5. P2 tests (navigation + wiring).
6. Remaining completeness items as separate scoped work.

Run `cargo fmt` and `cargo clippy` after each change.
