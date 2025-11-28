# Diff Search Feature Plan

## Goals
- Add in-diff search with live query updates, active match tracking, and navigation.
- Allow resuming search editing without losing query/flags; preserve state for session.
- Search covers all diff content, including collapsed hunks, and expands when needed.

## UX Flow
- Enter search: `/` opens bottom prompt; prefill last query/flags; live updates as user types.
- Exit edit: `Enter` leaves edit mode, keeps matches visible, and focuses active match.
- Resume edit: pressing `/` reopens prompt with current query/flags.
- Cancel: `Esc` clears query/matches and closes prompt.
- Navigation: `n`/`Ctrl+J` next, `N`/`Ctrl+K` previous. `Enter` after typing also jumps to active match.
- Flags while in search: `Ctrl+I` toggle case-sensitivity, `Ctrl+W` toggle whole-word, `Ctrl+R` toggle regex. Persist for session.

## Rendering
- Bottom prompt: show `Search: <query>` with status `(current/total)` and flag chips `[Aa]` insensitive, `[W]`, `[.*]`.
- Highlights: all matches with a light style; active match with stronger style. On navigation, auto-scroll to active.
- Collapsed hunks: annotate with badge like `(+X matches)` if matches hidden. Expanding happens when active match enters a collapsed hunk.

## Behavior Details
- Default matching: case-insensitive substring.
- Word mode: if regex off, wrap query with `\b...\b`; if regex on, either disable word toggle or wrap with `\b`—pick one consistent behavior (lean toward wrapping).
- Regex: on invalid pattern, show inline error and keep last valid matches.
- Empty query clears highlights/results.
- Session persistence: store query + flags in UI state; reused when reopening search.

## Data & State
- Extend diff view state with:
  - `search_query: String`
  - `search_flags: { case_sensitive, whole_word, regex }`
  - `search_matches: Vec<Match { hunk_id, line_idx, col, len }>`
  - `active_match_idx: Option<usize>`
  - `search_mode: enum { Inactive, Editing, Viewing }`
- Keep mapping from visual lines to underlying diff lines/hunks so navigation survives hunk expansion.

## Search Engine
- Source text: use diff data structure before render, including collapsed hunks.
- Matcher:
  - Build pattern per flags; precompile regex when needed.
  - For substring mode, use lowercased text for case-insensitive; handle word boundaries.
- Produce matches per line with positions for highlighting.

## Navigation Logic
- `next`/`prev` wrap around total matches; update `active_match_idx`.
- When new active match belongs to collapsed hunk, expand that hunk and reposition scroll.
- If matches change (query edit), reset active to first match.

## Error/Edge Cases
- Invalid regex: display message in prompt; keep previous matches.
- Large diffs: consider lightweight debounce; avoid blocking UI by limiting per-keystroke work to changed query.
- No matches: show `(0/0)` and clear active index; no auto-expand.

## Implementation Steps
1) Add search state structs/fields to diff view model; include persistence across activations.
2) Wire base key handlers for `/`, `Enter`, `Esc`, `n/N`, `Ctrl+J/K`; get edit/view/cancel flow working.
3) Implement matcher (substring/word/regex) and line-level match collection with positions.
4) Integrate search results into render: status bar, highlights, active scroll focus, badges for collapsed sections.
5) Hook navigation to active match focus and hunk auto-expand.
6) Add modifier toggles (`Ctrl+I/W/R`) after the base flow (steps 2–5) is solid, so they layer cleanly.
7) Handle regex errors and empty query clearing.
8) Test with diffs containing collapsed hunks, mixed casing, word boundaries, regex errors, and navigation wrapping.
