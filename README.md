# mahgit

`mahgit` is a keyboard-driven terminal Git interface inspired by Magit.

## Install

With Homebrew:

```bash
brew tap andreabergia/homebrew-tap
brew install mahgit
```

From source:

```bash
cargo install --path .
```

Run `mahgit` inside a Git repository. Use `mahgit --console` to print repository status without opening the terminal interface.

## Development

```bash
just ci
```

## Releasing

Install `cargo-dist` and `cargo-release`, then run:

```bash
scripts/release.sh patch --execute
```

The script runs the local preflight checks, updates the version, commits and tags it, and pushes the result. The tag triggers GitHub Actions to create the GitHub Release and update the Homebrew tap.

The repository must define a `HOMEBREW_TAP_TOKEN` Actions secret with write access to `andreabergia/homebrew-tap`.
