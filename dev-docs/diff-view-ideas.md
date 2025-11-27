# Diff View Improvement Ideas

## Current implementation snapshots
- `DiffView` renders hunks and lines as plain `Paragraph` content with basic coloring and no line numbers or inline decorations beyond a `+`/`-` prefix, and it renders the scrollbar only when the total lines exceed the viewport height. Selection highlighting is limited to the hunk header background when the hunk index matches. 【F:src/ui/diff_view.rs†L110-L184】
- Diff generation truncates diffs after 10k lines and aborts when encountering control characters, which protects the terminal but leaves the UI without a graceful way to show that truncation or let the user fetch more context. 【F:src/diff/generator.rs†L18-L123】【F:src/diff/generator.rs†L241-L310】

## UX improvements
- **Truncation messaging:** When `DiffGenerator` hits `MAX_LINES_PER_DIFF` or control-character checks, render a tail notice inside the diff (e.g., “Diff truncated after 10,000 lines – press `g` to load anyway”) instead of failing silently. Provide a follow-up action that reruns the diff with a higher limit just for that file.
- **Side-by-side view toggle:** Offer a 2-pane layout (old on left, new on right) for wider terminals, falling back to unified view on narrow widths. This would reuse the same hunk data but format lines differently.

## Navigation and accessibility
- **Search within diff:** Add a lightweight search mode to jump between matches in the current file, with highlights and a counter (e.g., `n`/`N` for next/previous).
- **Folding for context-only sections:** Allow collapsing unchanged context blocks beyond a threshold, leaving a placeholder line that can be expanded, which shrinks vertical space on large files.
- **Color-blind friendly palette:** Offer an alternate color scheme for additions/deletions (e.g., blue/orange) and avoid relying solely on color by adding symbols or markers in the gutter.

## Performance and robustness
- **Incremental rendering:** Stream hunks into the UI as they are parsed instead of buffering all lines, keeping the interface responsive on large diffs.
- **Lazy syntax highlighting:** Optionally feed diff lines through a syntax highlighter (e.g., `syntect`) with caching so that repeated toggles are fast but initial render does not block input.
- **Error surface area:** Surface binary/too-large errors inline with actionable tips, such as “press `o` to open in external pager/editor” when the in-app diff cannot render the file.
