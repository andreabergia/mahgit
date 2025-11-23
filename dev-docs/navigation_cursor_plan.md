# Navigation Cursor Refactor Plan

Goal: simplify and harden navigation by representing selection as a single cursor enum with only two variants (file or hunk), removing implicit section-header focus, and making arrow/j/k/PageUp/PageDown behavior predictable.

## Cursor Model
- Define `SelectionCursor` (likely in `ui/mod.rs`):
  - `File { section: StatusSection, file_index: usize }`
  - `Hunk { section: StatusSection, file_index: usize, hunk_index: usize }`
- App tracks only this cursor for selection/focus. `NavigationFocus` can be derived from the cursor (`File` -> File focus, `Hunk` -> InlineDiff focus).
- Add helpers:
  - `current_cursor() -> Option<SelectionCursor>` (build from navigation state; returns `None` only if no files).
  - `apply_cursor(cursor)` to mutate navigation state (set section/index, focus, hunk index).
  - `next_cursor(cursor)` / `previous_cursor(cursor)` for linear traversal over all items.
  - `first_file_cursor()` / `last_file_cursor()` convenience for edges and initial Down/Up.
- Remove/ignore section-header selection entirely (no `SectionHeader` focus, no header highlighting).

## Traversal Rules
- Linear order per section: File entry, then its hunks (if diff expanded and has hunks), then next file…; sections are ordered Conflicted -> Unstaged -> Untracked -> Staged (current order).
- `next_cursor`:
  - File -> first hunk if expanded+nonempty else next file; if no more files, go to first file of next non-empty section; if none, return `None`.
  - Hunk -> next hunk in file; if last hunk, go to next file as above.
- `previous_cursor`:
  - File -> previous file’s last hunk if expanded+nonempty, else previous file; if none, go to last file of previous non-empty section.
  - Hunk -> previous hunk; if first hunk, go to the owning file.
- Initial state: cursor should be first available file (if any) so Down works immediately.

## Key Behavior
- Up/Down and j/k: move selection using `previous_cursor` / `next_cursor`. Always change selection; no manual-scroll fallback.
- Left/Right: scroll viewport only by half a page; do not change cursor. Manual scroll should not re-anchor selection.
- PageUp/PageDown: scroll viewport only by a full page; do not change cursor. Manual scroll should not re-anchor selection.
- Shift+J/K (hunk jumps): go to first/last element in current parent, i.e. go to first/last hunk if viewing hunks of a file, or first/last file of a section.
- gg/G unchanged: top -> first file, bottom -> last file (consider last hunk only if you want; simplest is last file).

## Rendering Expectations
- Selection is determined by cursor:
  - If cursor is File: highlight file row.
  - If cursor is Hunk: highlight that hunk header; file row not selected.
- Section headers are never selected; they shouldn’t block Up/Down.
- List offset should respect manual scroll; when manual scroll active, do not re-apply selection anchoring on render.

## Data/State Cleanup
- Remove `NavigationFocus::SectionHeader` and related code paths/highlighting.
- Make `NavigationFocus` two-state (`File`, `InlineDiff`), or derive focus on the fly from cursor to reduce desync risk.
- Remove now-unused helpers (`advance_from_*`, section-header focus setters) once cursor is authoritative.
- Ensure `current_inline_diff_info` uses cursor state or consistent navigation state; consider fetching diff/hunk info from applied cursor, not the other way around.

## Tests to Update/Add (later)
- Up/Down with no expanded diffs moves file-to-file.
- Up/Down with multiple expanded files and hunks traverses hunks then files.
- Left/Right stay within depth (file-only or hunk-only).
- Initial Down selects first file.
- PageUp/PageDown scroll without moving selection.

## Implementation Tips
- Implement cursor helpers first; refactor command handlers to use them (MoveUp/MoveDown, PreviousHunk/NextHunk, Shift+J/K).
- Centralize state mutation in `apply_cursor` to avoid drift between navigation and cursor.
- Keep assertions for impossible states (missing file index, empty hunk lists when cursor says Hunk).
