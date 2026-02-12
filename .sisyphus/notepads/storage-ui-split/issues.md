# Issues

- 2026-02-13: LSP briefly reported type-inference errors in UI files after rewiring imports to `zova_storage`; a full `cargo check -p zova` confirmed the dependency graph and resolved diagnostics.
- 2026-02-13: Existing mixed staged/unstaged changes made the crate-directory rename surface as old-path deletions plus new-path untracked files in `git status`; verification by build/tests confirmed functional correctness.
