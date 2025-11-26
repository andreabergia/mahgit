# Configuration System Design

## Overview

This document describes the design and implementation plan for mahgit's configuration system, which provides user-customizable settings for themes and display preferences.

## Goals

- **Cross-platform config location:** Use OS-appropriate config directories (`~/.config/mahgit/config.toml` on Unix, `%APPDATA%\mahgit\config.toml` on Windows)
- **Theme system:** Support well-known color schemes with both 16-color (terminal-safe) and 24-bit RGB (modern) variants
- **Display customization:** Allow configuration of tab width for diff rendering
- **Sensible defaults:** Work out-of-the-box with no configuration file required
- **Strict validation:** Exit with clear error messages on invalid configuration rather than silently falling back
- **Partial configs:** Merge user-provided partial configuration with defaults seamlessly
- **Simple implementation:** Single config file, no per-repository overrides

## Configuration File Format

### Location

Platform-specific paths using the `dirs` crate:
- **Unix/Linux/macOS:** `~/.config/mahgit/config.toml`
- **Windows:** `%APPDATA%\mahgit\config.toml` (e.g., `C:\Users\Username\AppData\Roaming\mahgit\config.toml`)

### Format: TOML

```toml
# Theme name - built-in themes available:
# - gruvbox-dark (16-color, dark background) [default]
# - gruvbox-light (16-color, light background)
# - github-dark (24-bit RGB, modern dark)
# - github-light (24-bit RGB, modern light)
theme = "gruvbox-dark"

# Tab display width in spaces (default: 4)
tab_width = 4
```

### Future Extension: Custom Color Overrides

For future implementation, the config format can be extended to support granular color customization:

```toml
theme = "gruvbox-dark"
tab_width = 4

# Optional custom color overrides
[theme.colors]
staged = "green"              # ANSI color name
unstaged = "#F85149"          # RGB hex
untracked = "#BC8CFF"         # RGB hex
selected_bg = "#2D333B"       # RGB hex
# ... additional color properties
```

## Theme System

### Theme Naming Convention

Following conventions established by popular terminal tools (delta, bat, fish), mahgit uses ecosystem-standard theme names rather than invented conventions. This makes the behavior immediately clear to users familiar with other tools.

### Initial Theme Set

**gruvbox-dark** (default)
- Based on the popular Gruvbox color scheme
- Uses 16 ANSI colors for maximum terminal compatibility
- Dark background optimized
- Colors:
  - Staged: Green (#98971A or ANSI green)
  - Unstaged: Red (#CC241D or ANSI red)
  - Untracked: Magenta (#B16286 or ANSI magenta)
  - Selection: White on DarkGray

**gruvbox-light**
- Light background variant of Gruvbox
- Uses 16 ANSI colors
- Suitable for light terminal themes

**github-dark**
- Modern 24-bit RGB colors matching GitHub's dark theme
- Requires terminal with true color support
- Colors:
  - Staged: #3FB950 (GitHub green)
  - Unstaged: #F85149 (GitHub red)
  - Untracked: #BC8CFF (GitHub purple)
  - Selection: #2D333B background

**github-light**
- Light variant matching GitHub's light theme
- 24-bit RGB colors
- Dark text on light background

### Color Properties

Each theme defines colors for:
- `staged` - Staged file names and added lines
- `unstaged` - Unstaged file names and deleted lines
- `untracked` - Untracked file names
- `conflicted` - Conflicted file names
- `selected_fg` - Selected item foreground
- `selected_bg` - Selected item background
- `section_header` - Section headers (bold cyan in current implementation)
- `diff_context` - Context lines in diffs
- `diff_hunk_header` - Hunk headers
- `diff_hunk_header_focused` - Focused hunk header
- `diff_no_newline` - "No newline at end of file" marker
- `help_key` - Help overlay keybinding text
- `help_desc` - Help overlay description text

## Implementation Plan

### Module Structure

Single module approach: `src/config.rs`

Contains:
- `Config` struct with all configuration fields
- `ConfigError` enum for error types
- `Config::load()` method for loading and parsing
- `get_config_path()` helper for cross-platform paths
- Theme loading/validation logic

### Dependencies

Add to `Cargo.toml`:
```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
dirs = "5.0"
```

### Core Types

```rust
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub tab_width: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "gruvbox-dark".to_string(),
            tab_width: 4,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to determine config directory")]
    NoConfigDir,

    #[error("Failed to read config file {0}: {1}")]
    ReadError(PathBuf, std::io::Error),

    #[error("Failed to parse config file {0}: {1}")]
    ParseError(PathBuf, toml::de::Error),

    #[error("Unknown theme: {0}")]
    UnknownTheme(String),
}
```

### Loading Behavior

```rust
impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = get_config_path()?;

        // Missing config file is OK - use defaults
        if !config_path.exists() {
            return Ok(Self::default());
        }

        // Read and parse config file
        let contents = std::fs::read_to_string(&config_path)
            .map_err(|e| ConfigError::ReadError(config_path.clone(), e))?;

        // Parse TOML - serde will merge with defaults automatically
        let config: Config = toml::from_str(&contents)
            .map_err(|e| ConfigError::ParseError(config_path.clone(), e))?;

        // Validate theme exists
        validate_theme(&config.theme)?;

        Ok(config)
    }
}
```

### Error Handling in main.rs

```rust
fn main() {
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };

    // ... rest of initialization
}
```

**Behavior:**
- **Missing config file:** OK - use defaults, continue normally
- **Partial config file:** OK - serde with `#[serde(default)]` merges with defaults
- **Invalid TOML syntax:** ERROR - print to stderr, exit code 1
- **Unreadable file:** ERROR - print to stderr, exit code 1
- **Unknown theme name:** ERROR - print to stderr, exit code 1

## Tab Width Handling

### Current State

Currently, mahgit does not explicitly handle tab expansion. Tabs in diff content are rendered as-is to the terminal.

### Implementation Approach

**Display-time expansion:** Convert tabs to spaces during rendering based on the configured `tab_width`. This matches the behavior of modern diff tools and provides consistent display across terminals.

**Where to apply:**
- In diff content rendering (hunks and lines)
- Do NOT apply to file paths (file paths with tabs are extremely rare and should be displayed literally)

**Column tracking:** When expanding tabs, track the column position to expand to the next multiple of `tab_width`, matching standard tab stop behavior:
```rust
fn expand_tabs(line: &str, tab_width: usize) -> String {
    let mut result = String::with_capacity(line.len());
    let mut col = 0;

    for ch in line.chars() {
        if ch == '\t' {
            let spaces = tab_width - (col % tab_width);
            result.extend(std::iter::repeat(' ').take(spaces));
            col += spaces;
        } else {
            result.push(ch);
            col += 1;
        }
    }

    result
}
```

**Git diff prefix handling:** Git diffs include a prefix character (+/-/ ) before each line. Tab expansion should account for this prefix in column calculations.

### Integration Points

- Pass `tab_width` from `Config` to `App` struct
- Pass to `StatusView` and diff rendering functions
- Apply expansion in `render_diff_line()` or similar rendering functions
- Do NOT modify the stored diff content - only affect display

## Theme System Implementation

### Theme Definition

Create `src/theme.rs` module (or include in `config.rs`):

```rust
pub struct Theme {
    pub staged: Color,
    pub unstaged: Color,
    pub untracked: Color,
    pub conflicted: Color,
    pub selected_fg: Color,
    pub selected_bg: Color,
    pub section_header: Style,
    pub diff_context: Color,
    pub diff_hunk_header: Style,
    pub diff_hunk_header_focused: Style,
    pub diff_no_newline: Color,
    // ... additional properties
}

impl Theme {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "gruvbox-dark" => Some(Self::gruvbox_dark()),
            "gruvbox-light" => Some(Self::gruvbox_light()),
            "github-dark" => Some(Self::github_dark()),
            "github-light" => Some(Self::github_light()),
            _ => None,
        }
    }

    fn gruvbox_dark() -> Self {
        Self {
            staged: Color::Green,
            unstaged: Color::Red,
            untracked: Color::Magenta,
            conflicted: Color::Yellow,
            selected_fg: Color::White,
            selected_bg: Color::DarkGray,
            section_header: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            // ... rest of colors
        }
    }

    // ... other theme constructors
}
```

### Replacing Hardcoded Colors

Currently, colors are hardcoded throughout `ui/status_view.rs`, `ui/diff_view.rs`, and `ui/mod.rs` with inline `Style::default().fg(Color::*)` calls.

**Refactoring approach:**
1. Add `theme: Theme` field to `App` struct
2. Pass `&Theme` to rendering functions
3. Replace all hardcoded colors with theme references:
   ```rust
   // Before:
   Style::default().fg(Color::Green)

   // After:
   Style::default().fg(theme.staged)
   ```

**Migration:**
- Extract all 50+ color specifications into theme definitions
- Update `StatusView::render()`, `DiffView::render()`, and other rendering functions to accept `&Theme`
- Remove all hardcoded `Color::*` references from UI code

## Testing Strategy

### Unit Tests

- `Config::load()` with missing file returns default config
- `Config::load()` with partial config merges with defaults correctly
- `Config::load()` with invalid TOML returns parse error
- `Config::load()` with unknown theme returns error
- `expand_tabs()` correctly expands tabs at proper column positions

### Integration Tests

- Run mahgit with missing config file - should use defaults
- Run mahgit with valid config file - should apply settings
- Run mahgit with invalid config file - should exit with error code 1
- Verify each theme renders correctly in test repository

### Manual Testing

- Create `.tmp/test-tabs/` directory with files containing tabs
- Test each theme with `git diff` output
- Verify colors render correctly in different terminal emulators
- Test both dark and light themes
- Test 16-color themes in limited terminals
- Test 24-bit themes in modern terminals

## Future Enhancements

### Custom Color Overrides

Allow users to override specific colors while using a base theme:
```toml
theme = "gruvbox-dark"

[theme.colors]
staged = "#00ff00"
```

Implementation: Parse `theme.colors` table and apply overrides to the loaded theme.

### Theme Auto-Detection

Detect terminal background (light/dark) and automatically select appropriate theme variant. This could use the `terminal-colorsaurus` crate used by bat and delta.

### Additional Themes

Based on user feedback, add more popular themes:
- `nord` - Nordic-inspired color scheme
- `solarized-dark` / `solarized-light` - Classic Solarized themes
- `dracula` - Popular dark theme
- `monokai` - Monokai color scheme
- `one-dark` / `one-light` - Atom editor themes

### Configuration Validation Command

Add a command to validate configuration without running the full application:
```bash
mahgit config --validate
```

### Configuration Export

Add a command to export current default configuration:
```bash
mahgit config --init
# Creates ~/.config/mahgit/config.toml with all defaults
```

## References

- [Delta Configuration Documentation](https://dandavison.github.io/delta/configuration.html)
- [Delta Custom Themes](https://dandavison.github.io/delta/custom-themes.html)
- [Fish Shell Theme Configuration](https://fishshell.com/docs/current/cmds/fish_config.html)
- [Git Diff Tab Width Configuration](https://stackoverflow.com/questions/10581093/setting-tabwidth-to-4-in-git-show-git-diff)
- [bat Theme Support](https://github.com/sharkdp/bat)
- TOML Specification: https://toml.io/
- `dirs` crate: https://docs.rs/dirs/
