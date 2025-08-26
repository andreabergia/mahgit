use crate::status::RepositoryStatus;

pub fn display_status(status: &RepositoryStatus) {
    println!("On branch {}", status.branch_name);

    if status.is_clean() {
        println!("nothing to commit, working tree clean");
        return;
    }

    if !status.staged.is_empty() {
        println!("\nChanges to be committed:");
        println!("  (use \"git reset HEAD <file>...\" to unstage)");
        for file in &status.staged {
            println!("\t{}", file);
        }
    }

    if !status.unstaged.is_empty() {
        println!("\nChanges not staged for commit:");
        println!("  (use \"git add <file>...\" to update what will be committed)");
        for file in &status.unstaged {
            println!("\t{}", file);
        }
    }

    if !status.untracked.is_empty() {
        println!("\nUntracked files:");
        println!("  (use \"git add <file>...\" to include in what will be committed)");
        for file in &status.untracked {
            println!("\t{}", file);
        }
    }

    if !status.conflicted.is_empty() {
        println!("\nConflicted files:");
        println!("  (fix conflicts and run \"git add <file>...\" to mark resolution)");
        for file in &status.conflicted {
            println!("\t{}", file);
        }
    }
}
