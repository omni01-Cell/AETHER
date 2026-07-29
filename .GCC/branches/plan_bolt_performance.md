# Execution Plan: Bolt Performance Optimization

1. **Optimize `MultiTrackMixer::mix`**
   - Unswitch conditional `if track_ch == 1` to move it outside the sample loop.
   - Use `split_at_mut(1)` to get simultaneous mutable references to left and right channels of `mixed` buffer.
   - Use `.zip()` with iterators (`iter()`, `iter_mut()`) instead of indexing `track_data` and `mixed`.
   - Precalculate `left_gain` and `right_gain` outside the sample loop.
2. **Testing**
   - Ran `cargo test --workspace` to ensure functional parity and `cargo clippy --workspace` to check for lints.
3. **Log Learinngs**
   - Logged learning regarding `split_at_mut` and `.zip()` in `.jules/bolt.md`.
