## Diff Scrolling Behavior Proposal

Context: expanded inline diffs currently move the selection to the next hunk whenever the user presses an arrow key and the current hunk does not fit on screen. This makes it hard to review a single large hunk.

### Goals
- Make reviewing tall hunks predictable and smooth.
- Keep existing muscle memory for hunk-to-hunk navigation.
- Avoid new keybindings unless necessary.

### Proposed Key Behaviors

1. **Arrow keys (↑/↓)**
   - Scroll inline diffs by half the viewport height while keeping the current hunk selected.
   - If the next/previous hunk is already fully visible after the scroll, move the selection to it; otherwise stay on the current hunk.
   - Rationale: users can finish reading a hunk without sudden jumps, yet still advance quickly once the next hunk is on screen.

2. **Page Up / Page Down**
   - Always scroll by half a viewport with no automatic hunk selection changes.
   - Gives users coarse scrolling control that never affects selection state.

3. **`J` / `K`**
   - Always jump to the next / previous hunk respectively, regardless of visibility.
   - Keeps an explicit, reliable way to move between hunks rapidly.

### Implementation Notes
- Status view already tracks scroll offsets; extend `NavigationState` to store the viewport height and the first/last rendered row for each expanded hunk so we can test “is the next hunk fully visible?” after a scroll.
- Arrow and Page key handlers should call a shared helper (e.g., `scroll_inline_diff(by_lines: i16, change_selection: bool)`) that updates the viewport offset and optionally moves the selection.
- Need to ensure the general status list (outside inline diffs) keeps its current behavior; smart scrolling should activate only when the focus is inside an expanded inline diff.

### Decisions
- Half-screen scrolls should round up to ensure progress even on small viewports.
- The existing highlight for the selected hunk is sufficient feedback; no extra indicator is needed when the next hunk is visible.
- Horizontal scrolling improvements are out of scope for now.
