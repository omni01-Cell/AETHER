1. **Extract and unswitch the DSP mix loop in `MultiTrackMixer::mix`**
   - The loop in `crates/aether-audio/src/dsp.rs` (`MultiTrackMixer::mix`) processes audio samples and currently has an `if track_ch == 1` conditional inside the inner loop for every sample, along with repeated bounds checking.
   - I will use `replace_with_git_merge_diff` to:
     - Pre-calculate `vol * left_pan` and `vol * right_pan`.
     - Use `mixed.split_at_mut(1)` to get mutable references to the left and right output channels.
     - Unswitch the `track_ch == 1` condition out of the loop.
     - Replace the slice indexing with `.iter_mut().zip()` to elide runtime bounds checks completely.
     - Verify with `read_file`.
2. **Journal and GCC Updates**
   - Use `write_file` to create or update `.jules/bolt.md` with the new learning about DSP loop unswitching and bounds check elision.
   - Use `write_file` to create `.GCC/branches/plan_dsp_mix_optimization.md` containing the current plan.
   - Use `replace_with_git_merge_diff` to update `.GCC/main.md` with the new active branch `plan_dsp_mix_optimization`.
   - Use `run_in_bash_session` with `cat` to verify the creation and contents of these files.
3. **Compile and Run Tests**
   - Run `cargo test --workspace` via `run_in_bash_session` to ensure no functionality is broken by the refactoring.
   - Run `cargo clippy --workspace --all-targets -- -W clippy::perf` via `run_in_bash_session` to confirm there are no remaining perf issues in the modified block.
4. **Complete Pre-commit**
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
5. **Submit**
   - Use the `submit` tool with title `⚡ Bolt: [performance improvement]` and a description explaining What, Why, Impact, and Measurement.