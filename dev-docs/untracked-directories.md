# Plan for Showing Individual Files in New Directories

## Problem
Currently, when a new directory with files is created, mahgit shows the directory name in the untracked files section, similar to `git status`. The user wants to see all individual files within the new directory as separate untracked entries instead.

## Solution
Modify the `RepositoryStatus::new()` function in `src/status.rs` to expand directories in the untracked files list by adding all individual files within those directories.

## Implementation Steps

1. **Add directory expansion logic**:
   - Create a helper function that takes a path and expands directories to list all files within them
   - Use the `walkdir` crate that's already available in the project

2. **Modify the untracked files collection**:
   - After collecting untracked files from git2, process them to expand any directories
   - Replace directory entries with individual file entries

3. **Update the RepositoryStatus::new() method**:
   - Apply the directory expansion logic to the untracked files before storing them

4. **Test the changes**:
   - Run existing tests to ensure no regressions
   - Add new tests to verify the new behavior

## Files to Modify
- `src/status.rs` - Main implementation
- Potentially add tests to verify the behavior

## Dependencies
- The `walkdir` crate is already available in the project, so no new dependencies needed