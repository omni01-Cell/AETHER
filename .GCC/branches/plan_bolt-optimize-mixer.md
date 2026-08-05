1. **Optimize `MultiTrackMixer::mix` inner loop in `crates/aether-audio/src/dsp.rs`**
   - Use `replace_with_git_merge_diff` to replace the inner mixing loop in `MultiTrackMixer::mix`.
   - The optimization involves:
     - Hoisting the multiplication for left and right gains (`vol * left_pan` and `vol * right_pan`) outside the loop.
     - Extracting the mono/stereo conditional (`track_ch == 1`) outside the inner loop (loop unswitching).
     - Using `split_at_mut(1)` to borrow `mixed[0]` and `mixed[1]` simultaneously to avoid mutable aliasing.
     - Using iterators (`iter_mut().zip(...)`) instead of index-based access (`mixed[0][sample_idx]`) to elide runtime bounds checks.
     - Adding an inline comment `// Optimization (Bolt): ...` explaining the changes.
2. **Verify optimization**
   - Use `run_in_bash_session` to run `git diff` to verify the changes applied correctly.
   - Use `run_in_bash_session` to run `cargo test --workspace` to ensure no tests were broken.
   - Use `run_in_bash_session` to run `cargo clippy --workspace --all-targets` to ensure no new linting warnings.
   - Use `run_in_bash_session` to clean up temporary files by running `rm benches_compressor.rs benches_compressor benches_mixer.rs benches_mixer benches_biquad.rs benches_biquad`.
3. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
4. **Submit the PR**
   - Use `run_in_bash_session` to create a new branch (`git checkout -b bolt-optimize-mixer`), commit the changes (`git commit -m "..."`), and push/submit the PR via `gh pr create` with the title '⚡ Bolt: [performance improvement]' and include '💡 What', '🎯 Why', '📊 Impact', and '🔬 Measurement'.
