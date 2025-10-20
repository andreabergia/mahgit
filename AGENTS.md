# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview
This is "mahgit", a Rust-based terminal Git interface that aims to replicate Magit's functionality. It's a terminal-based Git interface with keyboard-driven navigation and operations.

## Key Design Goals
- Terminal-based Git interface replicating Magit's workflows
- Keyboard-driven navigation and Git operations
- Performance and memory safety via Rust
- Single binary deployment with no runtime dependencies
- Responsive UI during Git operations
- Cross-platform compatibility (Windows, macOS, Linux)

## Development Commands

### Build and Run
- `cargo build` - Build the project
- `cargo run` - Build and run the application
- `cargo check` - Fast check for compilation errors without building
- `cargo test` - Run tests

### Development Tools
- `cargo fmt` - Format code according to Rust standards
- `cargo clippy` - Run Rust linter for additional checks
- `cargo clean` - Clean build artifacts

**IMPORTANT**: Always run `cargo fmt` and `cargo clippy` after making any code changes and before committing. Claude Code should automatically run these commands after completing any coding task to ensure code quality and consistency.

### Testing and Development
- Use the `.tmp/` directory for temporary test files and repositories
- Never use `/tmp` - always use the project's `.tmp/` directory for testing

### Commit rules
Do not give Claude attribution in the message. Try to keep message concise.
Use conventional commits.

## Architecture and Technology Stack

**Core Dependencies (planned)**:
- `ratatui` - Terminal UI framework for widgets, layout management, and event handling
- `crossterm` - Cross-platform terminal manipulation for keyboard input and cursor management
- `git2` - Rust bindings for libgit2, providing comprehensive Git functionality

**Optional Future Dependencies**:
- `syntect` - Syntax highlighting for diff content
- `similar` - Advanced diff algorithms
- `tokio` - Async runtime for non-blocking Git operations
- `serde` - Configuration file support
- `clap` - Command-line argument parsing

## Claude guidelines

Always plan first then stop.
Never include weeks or similar estimates in plans.
