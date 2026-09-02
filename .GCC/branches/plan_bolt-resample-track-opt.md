# Plan: Optimize audio resampling inner loop

1. Use `replace_with_git_merge_diff` on `crates/aether-audio/src/dsp.rs` to replace expensive `.floor()` and `.ceil()` operations with safe integer casting and precalculate the inverse ratio.
2. Verify the changes using `git diff`.
3. Run tests and linter via `cargo test --workspace` and `cargo clippy`.
4. Update the bolt journal.
5. Register the plan in `.GCC/main.md`.
6. Complete pre-commit steps.
7. Submit PR.
